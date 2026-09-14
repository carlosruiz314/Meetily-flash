//! Full end-to-end re-transcription of a meeting, headless (chosen over
//! window-patching for whole-meeting healing — the user's call on
//! whisper-hallucination-cleanup 5.2). Mirrors `run_retranscription`'s
//! pipeline exactly — decode → 16 kHz mono → VAD (redemption
//! [`VAD_REDEMPTION_TIME_MS`]) → split >25 s segments at silence →
//! [`transcribe_segments_checkpointed`] (batch beam decode + the landed
//! hallucination quarantine strict retry) → clean-slate dual-table save —
//! with the two tauri-only pieces replaced: progress events become eprintln,
//! and the configured model comes from env instead of the settings table
//! (the app setting says large-v3, which is not on disk; this run uses the
//! on-disk turbo per the principal's decision).
//!
//! Report-only by default (transcribes everything, audits the new rows,
//! dumps them to JSON). RT_APPLY=1 additionally backs up the current rows of
//! BOTH tables plus the meeting folder's transcripts.json, then rewrites the
//! DB in ONE transaction — the production save path (clean-slate delete +
//! dual-write), so the immutable source is regenerated whole and every
//! segment goes through the fixed language resolution + quarantine.
//!
//! Ignored by default; run explicitly:
//!
//!   cargo test --release --test full_retranscribe_run -- --ignored --nocapture
//!
//! Environment:
//!   RT_MEETING_ID   meeting to re-transcribe (default: cde5c264)
//!   RT_DB           sqlite path (default: real app DB)
//!   RT_FOLDER       meeting folder containing audio.mp4 (required)
//!   RT_MODEL        model name (default: large-v3-turbo-q5_0)
//!   RT_MODELS_DIR   models dir (default: <appdata>/com.meetily.ai/models)
//!   RT_LANGUAGE     run language passed to the decode (default:
//!                   auto-translate — what resolve_batch_language produces
//!                   for the default UI preference)
//!   RT_APPLY=1      REWRITE the DB — close the app first

use app_lib::audio::common::{create_transcript_segments, split_segment_at_silence, write_transcripts_json};
use app_lib::audio::decoder::decode_audio_file;
use app_lib::audio::hallucination::audit;
use app_lib::audio::retranscription::{
    transcribe_segments_checkpointed, VAD_REDEMPTION_TIME_MS,
};
use app_lib::audio::vad::get_speech_chunks_with_progress;
use app_lib::database::repositories::transcript::TranscriptsRepository;
use app_lib::whisper_engine::WhisperEngine;
use std::path::PathBuf;
use std::sync::Arc;

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";

fn app_data_dir() -> PathBuf {
    dirs::data_dir().expect("no user data dir").join("com.meetily.ai")
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[tokio::test]
#[ignore = "full re-transcription of a REAL meeting (~hours on CPU); run explicitly with --ignored"]
async fn full_retranscribe_run() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();
    let t0 = std::time::Instant::now();

    let meeting_id = env_or("RT_MEETING_ID", MEETING_ID);
    let db = PathBuf::from(env_or("RT_DB", app_data_dir().join("meeting_minutes.sqlite").to_str().unwrap()));
    assert!(db.exists(), "DB not found at {}", db.display());
    let folder = PathBuf::from(
        std::env::var("RT_FOLDER")
            .expect("set RT_FOLDER to the meeting folder containing audio.mp4"),
    );
    let audio_path = folder.join("audio.mp4");
    assert!(audio_path.exists(), "audio not found at {}", audio_path.display());
    let language = env_or("RT_LANGUAGE", "auto-translate");
    let model = env_or("RT_MODEL", "large-v3-turbo-q5_0");
    let models_dir = PathBuf::from(env_or("RT_MODELS_DIR", app_data_dir().join("models").to_str().unwrap()));
    let apply = std::env::var("RT_APPLY").ok().as_deref() == Some("1");

    let pool = sqlx::sqlite::SqlitePool::connect(db.to_str().unwrap())
        .await
        .unwrap();

    // ── 1. Decode → 16 kHz mono → VAD → silence-boundary split ────────────
    let decoded = tokio::task::spawn_blocking({
        let audio_path = audio_path.clone();
        move || decode_audio_file(&audio_path)
    })
    .await
    .expect("decode task panicked")
    .expect("decode failed");
    let audio_samples = decoded.to_whisper_format();
    eprintln!(
        "[{:.0}s] decoded {:.1}s → {} whisper samples",
        t0.elapsed().as_secs_f64(),
        decoded.duration_seconds,
        audio_samples.len()
    );

    let speech_segments = get_speech_chunks_with_progress(
        &audio_samples,
        VAD_REDEMPTION_TIME_MS,
        |vad_progress, segments_found| {
            if vad_progress % 10 == 0 {
                eprintln!("[VAD {vad_progress}%] {segments_found} segments found");
            }
            !crate_should_cancel()
        },
    )
    .expect("VAD failed");
    eprintln!(
        "[{:.0}s] VAD: {} speech segments",
        t0.elapsed().as_secs_f64(),
        speech_segments.len()
    );

    const MAX_SEGMENT_SAMPLES: usize = 25 * 16000;
    let mut processable_segments: Vec<app_lib::audio::vad::SpeechSegment> = Vec::new();
    for segment in &speech_segments {
        if segment.samples.len() > MAX_SEGMENT_SAMPLES {
            let sub = split_segment_at_silence(segment, MAX_SEGMENT_SAMPLES);
            processable_segments.extend(sub);
        } else {
            processable_segments.push(segment.clone());
        }
    }
    eprintln!(
        "[{:.0}s] {} processable segments after splitting",
        t0.elapsed().as_secs_f64(),
        processable_segments.len()
    );

    // ── 2. The production transcribe loop (quarantine included) ───────────
    let engine = Arc::new(WhisperEngine::new_with_models_dir(Some(models_dir)).expect("engine init failed"));
    engine.discover_models().await.expect("model discovery failed");
    engine.load_model(&model).await.expect("model load failed");
    eprintln!("[{:.0}s] model {model} loaded", t0.elapsed().as_secs_f64());

    // Same strict-retry pin as run_retranscription: the run's concrete code,
    // else `en` (auto / auto-translate decode English-default).
    let strict_language: Option<String> = Some(match language.as_str() {
        "auto" | "auto-translate" => "en".to_string(),
        code => code.to_string(),
    });

    let (all_transcripts, total_confidence) = transcribe_segments_checkpointed(
        &meeting_id,
        &processable_segments,
        &pool,
        |i, segment| {
            let engine = engine.clone();
            let language = language.clone();
            let samples = segment.samples.clone();
            let segment_offset_ms = segment.start_timestamp_ms as i64;
            async move {
                let (text, conf, _, token_ts) = engine
                    .transcribe_audio_with_confidence(samples, Some(language), segment_offset_ms)
                    .await
                    .map_err(|e| anyhow::anyhow!("Whisper transcription failed on segment {}: {}", i, e))?;
                Ok((text, conf, token_ts))
            }
        },
        |segment| {
            let engine = engine.clone();
            let strict_language = strict_language.clone();
            let samples = segment.samples.clone();
            let segment_offset_ms = segment.start_timestamp_ms as i64;
            async move {
                engine
                    .transcribe_audio_strict(samples, strict_language, segment_offset_ms)
                    .await
                    .map_err(|e| anyhow::anyhow!("strict retry failed: {}", e))
            }
        },
        |done, total| {
            if done % 25 == 0 || done == total {
                eprintln!(
                    "[{:.0}s] transcribed {done}/{total} segments",
                    t0.elapsed().as_secs_f64()
                );
            }
        },
    )
    .await
    .expect("checkpointed transcription failed");
    let transcribed = all_transcripts.len();
    eprintln!(
        "[{:.0}s] transcribed {transcribed} segments, avg confidence {:.2}",
        t0.elapsed().as_secs_f64(),
        total_confidence / transcribed.max(1) as f32
    );

    // ── 3. Audit the fresh rows (quarantine should have left zero garbage) ─
    let mut flagged_new = Vec::new();
    for (text, start_ms, end_ms, _) in &all_transcripts {
        if audit(text, *start_ms, *end_ms).is_garbage {
            flagged_new.push((text.clone(), *start_ms, *end_ms));
        }
    }
    eprintln!(
        "AUDIT OF FRESH ROWS: {}/{} flagged (expect 0 — the quarantine runs inside the loop)",
        flagged_new.len(),
        transcribed
    );
    for (text, start_ms, end_ms) in &flagged_new {
        eprintln!("  STILL FLAGGED [{start_ms:.0}ms–{end_ms:.0}ms] {text}");
    }

    let new_segments = create_transcript_segments(&all_transcripts);

    if !apply {
        let dump_path = db.with_extension(format!(
            "retranscribe-report-{}.json",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));
        let dump = serde_json::json!({
            "meeting_id": meeting_id,
            "model": model,
            "language": language,
            "segments": new_segments.iter().map(|s| serde_json::json!({
                "text": s.text,
                "start_s": s.audio_start_time,
                "end_s": s.audio_end_time,
            })).collect::<Vec<_>>(),
        });
        std::fs::write(&dump_path, serde_json::to_string_pretty(&dump).unwrap())
            .expect("report dump failed");
        eprintln!(
            "report-only run: fresh rows dumped to {} (set RT_APPLY=1, app closed, to write)",
            dump_path.display()
        );
        return;
    }

    // ── 4. Backup, then the production clean-slate dual-table save ────────
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");

    // Render columns as text (SQLite coerces back on restore; the DB itself
    // plus transcripts.json stay the authoritative restore points).
    async fn dump_table(
        pool: &sqlx::SqlitePool,
        sql: &str,
        meeting_id: &str,
        cols: &[&str],
    ) -> Vec<std::collections::BTreeMap<String, Option<String>>> {
        let rows = sqlx::query(sql).bind(meeting_id).fetch_all(pool).await.unwrap();
        rows.iter()
            .map(|r| {
                cols.iter()
                    .map(|c| {
                        (
                            c.to_string(),
                            sqlx::Row::try_get::<Option<String>, _>(r, c).unwrap_or(None),
                        )
                    })
                    .collect::<std::collections::BTreeMap<_, _>>()
            })
            .collect()
    }

    let transcript_cols = [
        "id", "transcript", "timestamp", "summary", "action_items", "key_points", "speaker",
        "audio_start_time", "audio_end_time", "duration", "token_timestamps", "speaker_label",
        "speaker_source", "previous_label",
    ];
    let transcripts_sql = "SELECT id, transcript, timestamp, summary, action_items, key_points, \
         speaker, CAST(audio_start_time AS TEXT), CAST(audio_end_time AS TEXT), CAST(duration AS TEXT), \
         token_timestamps, speaker_label, speaker_source, previous_label \
         FROM transcripts WHERE meeting_id = ?";
    let source_cols = [
        "id", "transcript", "timestamp", "summary", "action_items", "key_points", "speaker",
        "audio_start_time", "audio_end_time", "duration", "token_timestamps", "source_origin",
    ];
    let sources_sql = "SELECT id, transcript, timestamp, summary, action_items, key_points, \
         speaker, CAST(audio_start_time AS TEXT), CAST(audio_end_time AS TEXT), CAST(duration AS TEXT), \
         token_timestamps, source_origin \
         FROM transcript_sources WHERE meeting_id = ?";

    let backup = serde_json::json!({
        "meeting_id": meeting_id,
        "taken_at": chrono::Utc::now().to_rfc3339(),
        "transcripts": dump_table(&pool, transcripts_sql, &meeting_id, &transcript_cols).await,
        "transcript_sources": dump_table(&pool, sources_sql, &meeting_id, &source_cols).await,
    });
    let backup_path = db.with_extension(format!("retranscribe-backup-{ts}.json"));
    std::fs::write(&backup_path, serde_json::to_string_pretty(&backup).unwrap())
        .expect("backup write failed");
    eprintln!("originals backed up to {}", backup_path.display());

    let json_backup = folder.join(format!("transcripts.pre-retranscribe-{ts}.json"));
    if folder.join("transcripts.json").exists() {
        std::fs::copy(folder.join("transcripts.json"), &json_backup)
            .expect("transcripts.json backup failed");
        eprintln!("folder transcripts.json backed up to {}", json_backup.display());
    }

    let mut conn = pool.acquire().await.unwrap();
    let mut tx = sqlx::Connection::begin(&mut *conn).await.unwrap();
    sqlx::query("DELETE FROM transcripts WHERE meeting_id = ?")
        .bind(&meeting_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM transcript_sources WHERE meeting_id = ?")
        .bind(&meeting_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    for segment in &new_segments {
        TranscriptsRepository::insert_transcription_row(&mut *tx, &segment.id, &meeting_id, segment)
            .await
            .unwrap();
    }
    sqlx::Transaction::commit(tx).await.unwrap();

    // Production cleanup: the scratch checkpoints have served their purpose.
    sqlx::query("DELETE FROM retranscription_checkpoints WHERE meeting_id = ?")
        .bind(&meeting_id)
        .execute(&pool)
        .await
        .unwrap();

    if let Err(e) = write_transcripts_json(&folder, &new_segments) {
        eprintln!("warn: failed to write transcripts.json: {e}");
    }

    eprintln!(
        "WROTE {} fresh rows into BOTH tables for {meeting_id} ([{:.0}s] total). \
         Next: Speakers run re-derives labels from the regenerated source.",
        new_segments.len(),
        t0.elapsed().as_secs_f64()
    );
    assert!(flagged_new.is_empty(), "quarantine left flagged rows — do not ship");
    pool.close().await;
}

/// The headless run is never cancelled and never yields (no scheduler).
fn crate_should_cancel() -> bool {
    false
}
