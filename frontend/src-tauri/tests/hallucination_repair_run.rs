//! Offline hallucination repair harness (whisper-hallucination-cleanup
//! tasks 4.1/4.2), source-first per the post-`align-from-immutable-source`
//! revision.
//!
//! The AUDIT reads `transcript_sources` — the durable truth the speaker
//! pipeline aligns from; a rendering-only audit misses source-only garbage.
//! Every flagged source row is mapped to its rendering counterpart(s) by
//! span overlap for the report; a fully-absorbed row has NO counterpart and
//! is reported as such rather than skipped silently.
//!
//! The WRITE (task 4.2, `REPAIR_WRITE=1`) replaces rows in BOTH tables in
//! one transaction: `transcript_sources` gets the repaired text as fresh
//! transcription output (fresh decode's token JSON kept,
//! `source_origin = 'stt'`); `transcripts` gets the same text with
//! `token_timestamps` NULL and speaker columns cleared. A re-decoded window
//! yielding N clean segments writes N rows. Known transient, stated in the
//! report: a rendering row spanning the window AND a neighboring clean
//! source row is deleted wholesale (regenerated rendering rows span source
//! rows post-assembly) — the neighbor's text is transiently absent from the
//! rendering until the follow-up Speakers run re-derives everything from
//! the source, which is never touched beyond the flagged rows.
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
//!   REPAIR_MODEL        model name (default: large-v3; the app's on-disk
//!                       model is large-v3-turbo-q5_0 under
//!                       %APPDATA%/com.meetily.ai/models — pass
//!                       REPAIR_MODEL=large-v3-turbo-q5_0 and
//!                       REPAIR_MODELS_DIR=<appdata>/com.meetily.ai/models)
//!   REPAIR_MODELS_DIR   models dir (default: <manifest>/../models)
//!   REPAIR_LANGUAGE     pinned retry language (default: en)
//!   REPAIR_REPORT       optional path to save the report (task 5.2)
//!   REPAIR_WRITE=1      REWRITE the DB — close the app first; originals are
//!                       dumped to <db>.repair-<ts>.json before the transaction

use app_lib::audio::decoder::decode_audio_file;
use app_lib::audio::hallucination::{audit, HallucinationReport};
use app_lib::audio::speaker::alignment::{AlignedSegment, SpeakerSource};
use app_lib::database::repositories::speaker::SpeakerRepository;
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

#[derive(Debug, Clone)]
struct SourceRow {
    id: String,
    text: String,
    timestamp: String,
    start_s: f64,
    end_s: f64,
    tokens: Option<String>,
    origin: String,
}

#[derive(Debug, Clone)]
struct RenderRow {
    id: String,
    text: String,
    start_s: Option<f64>,
    end_s: Option<f64>,
    speaker_label: Option<String>,
    speaker_source: Option<String>,
    previous_label: Option<String>,
    tokens: Option<String>,
}

async fn load_source_rows(pool: &sqlx::SqlitePool, meeting_id: &str) -> Vec<SourceRow> {
    let rows: Vec<(String, String, String, Option<f64>, Option<f64>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT id, transcript, timestamp, audio_start_time, audio_end_time, \
             token_timestamps, source_origin \
             FROM transcript_sources WHERE meeting_id = ? ORDER BY audio_start_time, id",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "reading transcript_sources failed ({e}) — run the app once (or \
                 migrate_production_and_verify_source_parity) so the source table exists"
            )
        });
    rows.into_iter()
        .map(|(id, text, timestamp, start, end, tokens, origin)| SourceRow {
            id,
            text,
            timestamp,
            start_s: start.unwrap_or(0.0),
            end_s: end.unwrap_or(0.0),
            tokens,
            origin,
        })
        .collect()
}

async fn load_render_rows(pool: &sqlx::SqlitePool, meeting_id: &str) -> Vec<RenderRow> {
    let rows: Vec<(String, String, Option<f64>, Option<f64>, Option<String>, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT id, transcript, audio_start_time, audio_end_time, speaker_label, \
             speaker_source, previous_label, token_timestamps \
             FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time, id",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
        .unwrap();
    rows.into_iter()
        .map(
            |(id, text, start, end, speaker_label, speaker_source, previous_label, tokens)| {
                RenderRow {
                    id,
                    text,
                    start_s: start,
                    end_s: end,
                    speaker_label,
                    speaker_source,
                    previous_label,
                    tokens,
                }
            },
        )
        .collect()
}

fn spans_overlap(a_start: f64, a_end: f64, b_start: f64, b_end: f64) -> bool {
    a_start < b_end && a_end > b_start
}

/// Char-boundary-safe preview (garbage text is multi-byte salad; byte
/// slicing panics mid-codepoint).
fn clip(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

/// One flagged source window plus its replacement plan.
struct Repair {
    source: SourceRow,
    report: HallucinationReport,
    /// Span-overlapping rendering rows — report/backup context only.
    counterparts: Vec<RenderRow>,
    /// None → the flagged row is dropped entirely; Some → one replacement
    /// row per clean re-decoded segment.
    replacement: Option<Vec<ReplacementRow>>,
}

struct ReplacementRow {
    text: String,
    start_s: f64,
    end_s: f64,
    tokens: Option<String>,
}

/// The dual-table write (task 4.2), shared by the live run and the
/// integration test. One transaction: for every repair, delete the flagged
/// source row and every rendering row overlapping its span, then insert the
/// replacement rows — source rows as fresh STT output (text + token JSON +
/// `source_origin = 'stt'`), rendering rows with `token_timestamps` NULL and
/// no speaker columns. Returns (source rows written, rendering rows deleted).
async fn apply_repairs(
    pool: &sqlx::SqlitePool,
    meeting_id: &str,
    repairs: &[Repair],
) -> anyhow::Result<(usize, usize)> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Connection::begin(&mut *conn).await?;
    let mut written = 0usize;
    let mut rendering_deleted = 0usize;

    for r in repairs {
        sqlx::query("DELETE FROM transcript_sources WHERE id = ? AND meeting_id = ?")
            .bind(&r.source.id)
            .bind(meeting_id)
            .execute(&mut *tx)
            .await?;
        let removed = sqlx::query(
            "DELETE FROM transcripts WHERE meeting_id = ? \
             AND audio_start_time < ? AND audio_end_time > ?",
        )
        .bind(meeting_id)
        .bind(r.source.end_s)
        .bind(r.source.start_s)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        rendering_deleted += removed as usize;

        if let Some(rows) = &r.replacement {
            for row in rows {
                let id = format!("transcript-{}", uuid::Uuid::new_v4());
                let timestamp = chrono::Utc::now().to_rfc3339();
                let duration = row.end_s - row.start_s;
                // Fresh transcription output — the source row IS STT output,
                // so it keeps the fresh decode's token JSON and provenance.
                sqlx::query(
                    "INSERT INTO transcript_sources (id, meeting_id, transcript, timestamp, \
                     audio_start_time, audio_end_time, duration, token_timestamps, source_origin) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'stt')",
                )
                .bind(&id)
                .bind(meeting_id)
                .bind(&row.text)
                .bind(&timestamp)
                .bind(row.start_s)
                .bind(row.end_s)
                .bind(duration)
                .bind(&row.tokens)
                .execute(&mut *tx)
                .await?;
                // Rendering copy: same text; token timestamps and speaker
                // columns stay NULL — the Speakers run labels from source.
                sqlx::query(
                    "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, \
                     audio_start_time, audio_end_time, duration, token_timestamps) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, NULL)",
                )
                .bind(&id)
                .bind(meeting_id)
                .bind(&row.text)
                .bind(&timestamp)
                .bind(row.start_s)
                .bind(row.end_s)
                .bind(duration)
                .execute(&mut *tx)
                .await?;
                written += 1;
            }
        }
    }

    sqlx::Transaction::commit(tx).await?;
    Ok((written, rendering_deleted))
}

/// The live run. Report-only by default; `REPAIR_WRITE=1` rewrites.
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

    // ── 1. Audit the SOURCE rows (the durable truth) ────────────────────────
    let sources = load_source_rows(&pool, &meeting_id).await;
    assert!(!sources.is_empty(), "no transcript_sources rows for {meeting_id}");
    let renders = load_render_rows(&pool, &meeting_id).await;
    eprintln!(
        "meeting {meeting_id}: {} source rows, {} rendering rows",
        sources.len(),
        renders.len()
    );

    let mut repairs: Vec<Repair> = Vec::new();
    for s in &sources {
        if s.start_s <= 0.0 && s.end_s <= 0.0 {
            continue; // no time span → nothing to re-decode
        }
        let report = audit(&s.text, s.start_s * 1000.0, s.end_s * 1000.0);
        if !report.is_garbage {
            continue;
        }
        let mut counterpart_ids = Vec::new();
        let counterparts: Vec<RenderRow> = renders
            .iter()
            .filter(|r| match (r.start_s, r.end_s) {
                (Some(rs), Some(re)) => spans_overlap(s.start_s, s.end_s, rs, re),
                _ => false,
            })
            .inspect(|r| counterpart_ids.push(r.id.clone()))
            .cloned()
            .collect();
        eprintln!(
            "  FLAGGED [{:.2}s–{:.2}s] non_latin={} fffd={} loop={:.2}x{} wps={:.1} | {}",
            s.start_s,
            s.end_s,
            report.non_latin,
            report.fffd,
            report.loop_ratio,
            report.loop_count,
            report.wps,
            clip(&s.text, 90)
        );
        eprintln!(
            "    rendering counterpart(s): {}",
            if counterpart_ids.is_empty() {
                "none — fully-absorbed source row (no rendering counterpart)".to_string()
            } else {
                counterpart_ids.join(", ")
            }
        );
        repairs.push(Repair {
            source: s.clone(),
            report,
            counterparts,
            replacement: None,
        });
    }
    eprintln!("flagged {}/{} source rows", repairs.len(), sources.len());
    if repairs.is_empty() {
        eprintln!("nothing to repair");
        return;
    }

    // ── 2. One decode; strict per-segment re-transcription of the windows ──
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

    for r in &mut repairs {
        let win_start = (r.source.start_s * 1000.0) as i64;
        let win_end = (r.source.end_s * 1000.0) as i64;
        let segments = engine
            .transcribe_audio_strict_segments(
                samples[win_start as usize * 16..(win_end as usize * 16).min(samples.len())].to_vec(),
                Some(language_pin.clone()),
                win_start,
                win_end,
            )
            .await
            .expect("strict decode failed");
        let clean: Vec<ReplacementRow> = segments
            .into_iter()
            .filter(|seg| {
                !audit(&seg.text, seg.start_ms as f64, seg.end_ms as f64).is_garbage
            })
            .map(|seg| ReplacementRow {
                text: seg.text,
                start_s: seg.start_ms as f64 / 1000.0,
                end_s: seg.end_ms as f64 / 1000.0,
                tokens: seg.token_timestamps,
            })
            .collect();
        let total = if clean.is_empty() { 0 } else { clean.len() };
        eprintln!(
            "  {} [{:.2}s–{:.2}s] old: {}",
            if total == 0 { "DROPPED " } else { "REPAIRED" },
            r.source.start_s,
            r.source.end_s,
            clip(&r.source.text, 110)
        );
        for c in &clean {
            eprintln!(
                "    → [{:.2}s–{:.2}s] {}",
                c.start_s,
                c.end_s,
                clip(&c.text, 110)
            );
        }
        r.replacement = if total == 0 { None } else { Some(clean) };
    }

    let repaired_windows = repairs.iter().filter(|r| r.replacement.is_some()).count();
    let dropped = repairs.iter().filter(|r| r.replacement.is_none());
    let written_estimate: usize = repairs
        .iter()
        .filter_map(|r| r.replacement.as_ref().map(|v| v.len()))
        .sum();
    eprintln!(
        "OUTCOME: {written_estimate} replacement row(s) across {repaired_windows} repaired window(s), \
         {} dropped, of {} flagged",
        dropped.count(),
        repairs.len()
    );
    for d in repairs.iter().filter(|r| r.replacement.is_none()) {
        eprintln!(
            "  dropped text [{} | {:.2}s]: {}",
            d.source.id,
            d.source.start_s,
            d.source.text
        );
    }
    eprintln!(
        "NOTE (documented transient): rendering rows overlapping a repaired window are deleted \
         wholesale — a neighboring clean source row's rendering text is transiently absent until \
         the follow-up Speakers run re-derives the rendering from the healed source."
    );

    if let Ok(report_path) = std::env::var("REPAIR_REPORT") {
        let mut report = String::new();
        report.push_str(&format!(
            "# hallucination repair report — {meeting_id}\n\n\
             model: {model}  language pin: {language_pin}\n\n\
             flagged {} of {} source rows; {repaired_windows} repaired window(s), \
             {written_estimate} replacement row(s)\n\n",
            repairs.len(),
            sources.len()
        ));
        for r in &repairs {
            report.push_str(&format!(
                "- [{:.2}s–{:.2}s] `{}`\n  - triggers: non_latin={} fffd={} loop={:.2}x{} wps={:.1}\n",
                r.source.start_s,
                r.source.end_s,
                r.source.text,
                r.report.non_latin,
                r.report.fffd,
                r.report.loop_ratio,
                r.report.loop_count,
                r.report.wps
            ));
            report.push_str(&format!(
                "  - rendering counterparts: {}\n",
                if r.counterparts.is_empty() {
                    "none (fully-absorbed source row)".to_string()
                } else {
                    r.counterparts.iter().map(|c| c.id.as_str()).collect::<Vec<_>>().join(", ")
                }
            ));
            match &r.replacement {
                Some(rows) => {
                    for row in rows {
                        report.push_str(&format!(
                            "  - repaired → [{:.2}s–{:.2}s] `{}`\n",
                            row.start_s,
                            row.end_s,
                            row.text
                        ));
                    }
                }
                None => report.push_str("  - DROPPED (still flagged after strict re-decode)\n"),
            }
        }
        std::fs::write(&report_path, report).expect("report write failed");
        eprintln!("report saved to {report_path}");
    }

    // ── 3. Optional write: backup, then rewrite ONLY the touched rows ──────
    if !write {
        eprintln!("report-only run (set REPAIR_WRITE=1, app closed, to rewrite the DB)");
        return;
    }

    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let backup_path = db.with_extension(format!("repair-{ts}.json"));
    let backup = serde_json::json!({
        "meeting_id": meeting_id,
        "model": model,
        "language": language_pin,
        "repairs": repairs.iter().map(|r| serde_json::json!({
            "source": {
                "id": r.source.id, "text": r.source.text,
                "timestamp": r.source.timestamp,
                "start_s": r.source.start_s, "end_s": r.source.end_s,
                "token_timestamps": r.source.tokens, "source_origin": r.source.origin,
            },
            "rendering_counterparts": r.counterparts.iter().map(|c| serde_json::json!({
                "id": c.id, "text": c.text,
                "start_s": c.start_s, "end_s": c.end_s,
                "speaker_label": c.speaker_label, "speaker_source": c.speaker_source,
                "previous_label": c.previous_label, "token_timestamps": c.tokens,
            })).collect::<Vec<_>>(),
            "replacement": r.replacement.as_ref().map(|rows| rows.iter().map(|row|
                serde_json::json!({
                    "text": row.text, "start_s": row.start_s, "end_s": row.end_s,
                })).collect::<Vec<_>>()),
        })).collect::<Vec<_>>(),
    });
    std::fs::write(&backup_path, serde_json::to_string_pretty(&backup).unwrap())
        .expect("backup write failed");
    eprintln!("originals backed up to {}", backup_path.display());

    let (written, rendering_deleted) = apply_repairs(&pool, &meeting_id, &repairs)
        .await
        .expect("repair write failed");
    eprintln!(
        "WROTE {written} replacement row(s) into BOTH tables; {rendering_deleted} rendering row(s) \
         removed. Re-run this harness (audit must come back clean), then one Speakers run to \
         re-label from the healed source."
    );
    pool.close().await;
}

/// Task 4.2's integration test (non-ignored): the repair write lands in BOTH
/// tables of a production-shaped database, and a subsequent regeneration
/// persist — the render half a Speakers run drives — re-derives the repaired
/// text from the healed source without resurrecting the garbage. No whisper,
/// no embeddings: the strict re-decode and the alignment are stand-ins; the
/// write path (`apply_repairs`) and the persist path are the real code.
#[tokio::test]
async fn repair_write_lands_in_both_tables_and_regeneration_rederives_repaired_text() {
    let pool = sqlx::sqlite::SqlitePool::connect(":memory:").await.unwrap();
    // Production-shaped minimal DDL: the column set the repair write and
    // persist_regenerated_rendering touch (mirrors the migrations; NOT NULL
    // only on id/meeting_id/transcript/timestamp).
    sqlx::query(
        "CREATE TABLE transcripts (
            id TEXT PRIMARY KEY,
            meeting_id TEXT NOT NULL,
            transcript TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            summary TEXT, action_items TEXT, key_points TEXT, speaker TEXT,
            audio_start_time REAL, audio_end_time REAL, duration REAL,
            token_timestamps TEXT,
            speaker_label TEXT, speaker_source TEXT, previous_label TEXT
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TABLE transcript_sources (
            id TEXT PRIMARY KEY,
            meeting_id TEXT NOT NULL,
            transcript TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            summary TEXT, action_items TEXT, key_points TEXT, speaker TEXT,
            audio_start_time REAL, audio_end_time REAL, duration REAL,
            token_timestamps TEXT,
            source_origin TEXT NOT NULL DEFAULT 'stt'
        )",
    )
    .execute(&pool)
    .await
    .unwrap();

    let meet = "meeting-repair-test-0001";
    let garbage_text = "你好吗谢谢再见欢迎不明觉厉慢慢来"; // ≥3 CJK chars → non-Latin trigger
    let clean_a = "Good morning everyone, let's get started.";
    let clean_b = "That wraps the agenda for today.";
    // Self-guard: the fixture garbage actually trips the audit, the clean
    // rows don't.
    assert!(audit(garbage_text, 10_000.0, 14_000.0).is_garbage);
    assert!(!audit(clean_a, 0.0, 10_000.0).is_garbage);
    assert!(!audit(clean_b, 14_000.0, 20_000.0).is_garbage);

    async fn seed_both(
        pool: &sqlx::SqlitePool,
        meet: &str,
        id: &str,
        text: &str,
        start_s: f64,
        end_s: f64,
        tokens: Option<&str>,
        speaker: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time, \
             audio_end_time, duration, token_timestamps, speaker_label, speaker_source) \
             VALUES (?, ?, ?, '2026-07-25T00:00:00Z', ?, ?, ?, ?, ?, 'auto')",
        )
        .bind(id)
        .bind(meet)
        .bind(text)
        .bind(start_s)
        .bind(end_s)
        .bind(end_s - start_s)
        .bind(tokens)
        .bind(speaker)
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO transcript_sources (id, meeting_id, transcript, timestamp, \
             audio_start_time, audio_end_time, duration, token_timestamps, source_origin) \
             VALUES (?, ?, ?, '2026-07-25T00:00:00Z', ?, ?, ?, ?, 'stt')",
        )
        .bind(id)
        .bind(meet)
        .bind(text)
        .bind(start_s)
        .bind(end_s)
        .bind(end_s - start_s)
        .bind(tokens)
        .execute(pool)
        .await
        .unwrap();
    }

    seed_both(&pool, meet, "row-clean-1", clean_a, 0.0, 10.0, Some(r#"[{"word":"Good","start_ms":100,"end_ms":400}]"#), Some("Alice")).await;
    seed_both(&pool, meet, "row-garbage", garbage_text, 10.0, 14.0, None, Some("Bob")).await;
    seed_both(&pool, meet, "row-clean-2", clean_b, 14.0, 20.0, None, None).await;

    // ── Audit the source rows the way the harness does ──
    let sources = load_source_rows(&pool, meet).await;
    assert_eq!(sources.len(), 3);
    let mut repairs: Vec<Repair> = Vec::new();
    for s in &sources {
        let report = audit(&s.text, s.start_s * 1000.0, s.end_s * 1000.0);
        if report.is_garbage {
            repairs.push(Repair {
                source: s.clone(),
                report,
                counterparts: Vec::new(),
                replacement: None,
            });
        }
    }
    assert_eq!(repairs.len(), 1, "exactly the garbage source row flags");
    assert_eq!(repairs[0].source.id, "row-garbage");
    repairs[0].counterparts = vec![RenderRow {
        id: "row-garbage".to_string(),
        text: garbage_text.to_string(),
        start_s: Some(10.0),
        end_s: Some(14.0),
        speaker_label: Some("Bob".to_string()),
        speaker_source: Some("auto".to_string()),
        previous_label: None,
        tokens: None,
    }];

    // ── Strict re-decode stand-in: TWO clean segments (the N-rows case) ──
    repairs[0].replacement = Some(vec![
        ReplacementRow {
            text: "Could you share the numbers with us?".to_string(),
            start_s: 10.0,
            end_s: 12.0,
            tokens: Some(r#"[{"word":"Could","start_ms":10000,"end_ms":10400}]"#.to_string()),
        },
        ReplacementRow {
            text: "Sure, sending them right now.".to_string(),
            start_s: 12.0,
            end_s: 14.0,
            tokens: None,
        },
    ]);

    // ── The write: BOTH tables in one transaction ──
    let (written, rendering_deleted) = apply_repairs(&pool, meet, &repairs)
        .await
        .unwrap();
    assert_eq!(written, 2, "one replacement row per re-decoded segment");
    assert_eq!(rendering_deleted, 1, "the garbage rendering row is removed");

    // Source table: old id gone; two fresh 'stt' rows keeping the fresh
    // token JSON; neighbors untouched.
    let sources_after = load_source_rows(&pool, meet).await;
    assert_eq!(sources_after.len(), 4);
    assert!(!sources_after.iter().any(|s| s.id == "row-garbage"));
    let new_sources: Vec<&SourceRow> = sources_after
        .iter()
        .filter(|s| s.id != "row-clean-1" && s.id != "row-clean-2")
        .collect();
    assert_eq!(new_sources.len(), 2);
    assert!(new_sources.iter().all(|s| s.origin == "stt"));
    assert!(new_sources.iter().any(|s| s.tokens.is_some()), "fresh token JSON kept in the source");
    assert!(
        new_sources
            .iter()
            .all(|s| !audit(&s.text, s.start_s * 1000.0, s.end_s * 1000.0).is_garbage),
        "every replacement source row audits clean"
    );

    // Rendering: neighbors' speaker labels intact; the two fresh rows have
    // NULL tokens and no speaker columns.
    let renders_after = load_render_rows(&pool, meet).await;
    assert_eq!(renders_after.len(), 4);
    let alice = renders_after.iter().find(|r| r.id == "row-clean-1").unwrap();
    assert_eq!(alice.speaker_label.as_deref(), Some("Alice"));
    let fresh: Vec<&RenderRow> = renders_after
        .iter()
        .filter(|r| r.id != "row-clean-1" && r.id != "row-clean-2")
        .collect();
    assert_eq!(fresh.len(), 2);
    assert!(fresh.iter().all(|r| r.tokens.is_none()));
    assert!(fresh.iter().all(|r| r.speaker_label.is_none() && r.speaker_source.is_none()));

    // ── Subsequent align: regenerate the rendering from the healed source ──
    // (the persist half of a Speakers run, real production code)
    let healed = load_source_rows(&pool, meet).await;
    let aligned: Vec<AlignedSegment> = healed
        .iter()
        .map(|s| AlignedSegment {
            original_id: s.id.clone(),
            text: s.text.clone(),
            audio_start_ms: (s.start_s * 1000.0) as i64,
            audio_end_ms: (s.end_s * 1000.0) as i64,
            speaker: "Speaker 0".to_string(),
            speaker_source: SpeakerSource::Auto,
        })
        .collect();
    let regenerated =
        SpeakerRepository::persist_regenerated_rendering(&pool, meet, aligned, false)
            .await
            .unwrap();
    assert_eq!(regenerated, 4, "rendering rebuilt from all 4 source rows");

    let final_texts: Vec<String> = load_render_rows(&pool, meet)
        .await
        .into_iter()
        .map(|r| r.text)
        .collect();
    assert_eq!(final_texts.len(), 4);
    assert!(final_texts.iter().any(|t| t.contains("share the numbers")), "repaired text re-derived");
    assert!(final_texts.iter().any(|t| t.contains("sending them right now")));
    assert!(!final_texts.iter().any(|t| t == garbage_text), "garbage never resurrects");
    assert!(final_texts.iter().any(|t| t == clean_a) && final_texts.iter().any(|t| t == clean_b),
        "neighbor source rows survive regeneration");

    // And the source table is untouched by the regeneration run — the
    // headline invariant of align-from-immutable-source, now over healed data.
    assert_eq!(load_source_rows(&pool, meet).await.len(), 4);
    pool.close().await;
}
