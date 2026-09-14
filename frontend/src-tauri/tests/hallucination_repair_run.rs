//! Offline hallucination repair harness (whisper-hallucination-cleanup
//! tasks 4.1/4.2). Report-only by default; drives the SAME production pieces
//! as the batch lane: `audio::hallucination::audit` → strict re-decode of the
//! flagged windows (sample-sliced from ONE decode of the meeting audio) →
//! explicit, backed-up rewrite of only the replaced rows.
//!
//! Ignored by default; run explicitly:
//!
//!   cargo test --test hallucination_repair_run -- --ignored --nocapture
//!
//! Environment:
//!   REPAIR_MEETING_ID   meeting to repair (default: cde5c264)
//!   REPAIR_DB           sqlite path (default: real app DB)
//!   REPAIR_FOLDER       meeting folder containing audio.mp4 (required)
//!   REPAIR_AUDIO        audio file override (default: REPAIR_FOLDER/audio.mp4)
//!   REPAIR_MODEL        model name (default: large-v3)
//!   REPAIR_MODELS_DIR   models dir (default: <manifest>/../models)
//!   REPAIR_LANGUAGE     pinned retry language (default: en)
//!   REPAIR_WRITE=1      REWRITE the DB — close the app first; originals are
//!                       dumped to <db>.repair-<ts>.json before the transaction

use app_lib::audio::decoder::decode_audio_file;
use app_lib::audio::hallucination::audit;
use app_lib::whisper_engine::WhisperEngine;
use std::path::PathBuf;

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";

fn app_db_path() -> std::path::PathBuf {
    dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("meeting_minutes.sqlite")
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

#[tokio::test]
#[ignore = "drives the REAL app DB + REAL recording; run explicitly with --ignored"]
async fn hallucination_repair_run() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();

    let meeting_id = env_or("REPAIR_MEETING_ID", MEETING_ID);
    let db = PathBuf::from(env_or("REPAIR_DB", app_db_path().to_str().unwrap()));
    assert!(db.exists(), "DB not found at {}", db.display());
    let folder = PathBuf::from(
        std::env::var("REPAIR_FOLDER")
            .expect("set REPAIR_FOLDER to the meeting folder containing audio.mp4"),
    );
    let audio_path = std::env::var("REPAIR_AUDIO")
        .map(PathBuf::from)
        .unwrap_or_else(|_| folder.join("audio.mp4"));
    assert!(audio_path.exists(), "audio not found at {}", audio_path.display());
    let language_pin = env_or("REPAIR_LANGUAGE", "en");
    let model = env_or("REPAIR_MODEL", "large-v3");
    let write = std::env::var("REPAIR_WRITE").ok().as_deref() == Some("1");

    let pool = sqlx::sqlite::SqlitePool::connect(db.to_str().unwrap())
        .await
        .unwrap();

    // ── 1. Audit the stored rows ────────────────────────────────────────────
    let rows: Vec<(String, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "SELECT id, transcript, audio_start_time, audio_end_time \
         FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time, id",
    )
    .bind(&meeting_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(!rows.is_empty(), "no transcripts for {meeting_id}");
    eprintln!("meeting {meeting_id}: {} rows", rows.len());

    struct Flagged {
        id: String,
        text: String,
        start_ms: f64,
        end_ms: f64,
        report: app_lib::audio::hallucination::HallucinationReport,
    }
    let mut flagged = Vec::new();
    for (id, text, start, end) in &rows {
        let (start_ms, end_ms) = match (start, end) {
            (Some(s), Some(e)) => (*s * 1000.0, *e * 1000.0),
            _ => continue, // no time span → nothing to re-decode
        };
        let report = audit(text, start_ms, end_ms);
        if report.is_garbage {
            eprintln!(
                "  FLAGGED [{:.1}s–{:.1}s] non_latin={} fffd={} loop={:.2}x{} wps={:.1} | {}",
                start_ms / 1000.0,
                end_ms / 1000.0,
                report.non_latin,
                report.fffd,
                report.loop_ratio,
                report.loop_count,
                report.wps,
                &text[..text.len().min(90)]
            );
            flagged.push(Flagged {
                id: id.clone(),
                text: text.clone(),
                start_ms,
                end_ms,
                report,
            });
        }
    }
    eprintln!("flagged {}/{} rows", flagged.len(), rows.len());
    if flagged.is_empty() {
        eprintln!("nothing to repair");
        return;
    }

    // ── 2. One decode; strict re-transcription of the flagged windows ──────
    let decoded = tokio::task::spawn_blocking({
        let audio_path = audio_path.clone();
        move || decode_audio_file(&audio_path)
    })
    .await
    .expect("decode task panicked")
    .expect("decode failed");
    let samples = decoded.to_whisper_format();
    eprintln!(
        "decoded {:.1}s → {} whisper samples (16 kHz mono)",
        decoded.duration_seconds,
        samples.len()
    );

    let models_dir = PathBuf::from(env_or(
        "REPAIR_MODELS_DIR",
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../models")
            .to_string_lossy(),
    ));
    let engine = WhisperEngine::new_with_models_dir(Some(models_dir))
        .expect("engine init failed");
    engine.discover_models().await.expect("model discovery failed");
    engine.load_model(&model).await.expect("model load failed");

    struct Outcome {
        id: String,
        old: String,
        repaired: Option<String>, // None → dropped (still flagged after retry)
        start_ms: f64,
        end_ms: f64,
    }
    let mut outcomes = Vec::new();
    for f in &flagged {
        let lo = (f.start_ms * 16.0) as usize;
        let hi = ((f.end_ms * 16.0) as usize).min(samples.len());
        if hi <= lo {
            eprintln!("  SKIP {}: zero-length window", f.id);
            continue;
        }
        let (text, _conf, _ts) = engine
            .transcribe_audio_strict(samples[lo..hi].to_vec(), Some(language_pin.clone()), f.start_ms as i64)
            .await
            .expect("strict decode failed");
        let clean = !audit(&text, f.start_ms, f.end_ms).is_garbage;
        eprintln!(
            "  {} [{:.1}s–{:.1}s] {}",
            if clean { "REPAIRED" } else { "DROPPED " },
            f.start_ms / 1000.0,
            f.end_ms / 1000.0,
            &text[..text.len().min(110)]
        );
        outcomes.push(Outcome {
            id: f.id.clone(),
            old: f.text.clone(),
            repaired: clean.then_some(text),
            start_ms: f.start_ms,
            end_ms: f.end_ms,
        });
    }

    let repaired_n = outcomes.iter().filter(|o| o.repaired.is_some()).count();
    let dropped = outcomes.iter().filter(|o| o.repaired.is_none()).collect::<Vec<_>>();
    eprintln!(
        "OUTCOME: {} repaired-clean, {} dropped, of {} flagged",
        repaired_n,
        dropped.len(),
        outcomes.len()
    );
    for d in &dropped {
        eprintln!("  dropped text [{} | {:.1}s]: {}", d.id, d.start_ms / 1000.0, d.old);
    }

    // ── 3. Optional write: backup, then rewrite ONLY the touched rows ──────
    if std::env::var("REPAIR_WRITE").ok().as_deref() != Some("1") {
        eprintln!("report-only run (set REPAIR_WRITE=1, app closed, to rewrite the DB)");
        return;
    }

    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup_path = db.with_extension(format!("repair-{ts}.json"));
    let backup = serde_json::json!({
        "meeting_id": meeting_id,
        "model": model,
        "language": language_pin,
        "outcomes": outcomes.iter().map(|o| serde_json::json!({
            "id": o.id,
            "start_ms": o.start_ms,
            "end_ms": o.end_ms,
            "old": o.old,
            "new": o.repaired,
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&backup_path, serde_json::to_string_pretty(&backup).unwrap())
        .expect("backup write failed");
    eprintln!("originals backed up to {}", backup_path.display());

    let mut conn = pool.acquire().await.unwrap();
    let mut tx = sqlx::Connection::begin(&mut *conn).await.unwrap();
    for o in &outcomes {
        match &o.repaired {
            Some(new_text) => {
                sqlx::query(
                    "UPDATE transcripts SET transcript = ?, token_timestamps = NULL, \
                     speaker_label = NULL, previous_label = NULL \
                     WHERE id = ? AND meeting_id = ?",
                )
                .bind(new_text)
                .bind(&o.id)
                .bind(&meeting_id)
                .execute(&mut *tx)
                .await
                .unwrap();
                // The immutable source copy the speaker lane aligns from must
                // not re-serve the old text on the next Speakers run.
                sqlx::query(
                    "UPDATE transcript_sources SET transcript = ?, token_timestamps = NULL \
                     WHERE id = ? AND meeting_id = ?",
                )
                .bind(new_text)
                .bind(&o.id)
                .bind(&meeting_id)
                .execute(&mut *tx)
                .await
                .unwrap();
            }
            None => {
                sqlx::query("DELETE FROM transcripts WHERE id = ? AND meeting_id = ?")
                    .bind(&o.id)
                    .bind(&meeting_id)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
                sqlx::query("DELETE FROM transcript_sources WHERE id = ? AND meeting_id = ?")
                    .bind(&o.id)
                    .bind(&meeting_id)
                    .execute(&mut *tx)
                    .await
                    .unwrap();
            }
        }
    }
    sqlx::Transaction::commit(tx).await.unwrap();
    eprintln!(
        "WROTE {} repaired + {} dropped rows (re-run the audit; then Speakers to re-label)",
        repaired_n,
        dropped.len()
    );
}
