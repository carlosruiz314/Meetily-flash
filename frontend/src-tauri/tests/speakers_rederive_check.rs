//! Post-re-transcription acceptance (whisper-hallucination-cleanup 5.2,
//! final step): ONE Speakers run re-derives the rendering from the healed
//! `transcript_sources` — the immutable source must stay byte-identical
//! across the run (the align-from-immutable-source headline invariant, now
//! over healed data) and the re-derived rendering must contain zero
//! hallucination-flagged rows.
//!
//!   cargo test --test speakers_rederive_check -- --ignored --nocapture
//!
//! Environment: SRD_MEETING_ID (default cde5c264), SRD_DB (default app DB).

use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, sqlx::FromRow)]
struct SourceRow {
    id: String,
    transcript: String,
    audio_start_time: f64,
    audio_end_time: f64,
    token_timestamps: Option<String>,
}

fn source_hash(rows: &[SourceRow]) -> String {
    let mut sorted: Vec<&SourceRow> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        (
            (a.audio_start_time * 1000.0) as i64,
            (a.audio_end_time * 1000.0) as i64,
            a.id.as_str(),
        )
            .cmp(&(
                (b.audio_start_time * 1000.0) as i64,
                (b.audio_end_time * 1000.0) as i64,
                b.id.as_str(),
            ))
    });
    let canon: Vec<String> = sorted
        .iter()
        .map(|r| {
            format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                r.id,
                r.transcript,
                (r.audio_start_time * 1000.0) as i64,
                (r.audio_end_time * 1000.0) as i64,
                r.token_timestamps.as_deref().unwrap_or("")
            )
        })
        .collect();
    let mut h = Sha256::new();
    h.update(canon.join("\u{1e}").as_bytes());
    format!("{:x}", h.finalize())
}

async fn read_source(pool: &sqlx::SqlitePool, meeting_id: &str) -> Vec<SourceRow> {
    sqlx::query_as::<_, SourceRow>(
        "SELECT id, transcript, audio_start_time, audio_end_time, token_timestamps \
         FROM transcript_sources WHERE meeting_id = ? ORDER BY id",
    )
    .bind(meeting_id)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "drives the REAL app DB (Speakers recompute); run explicitly with --ignored"]
async fn speakers_rederive_check() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();

    let meeting_id = std::env::var("SRD_MEETING_ID")
        .unwrap_or_else(|_| "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323".to_string());
    let db = std::env::var("SRD_DB").unwrap_or_else(|_| {
        dirs::data_dir()
            .unwrap()
            .join("com.meetily.ai/meeting_minutes.sqlite")
            .to_string_lossy()
            .to_string()
    });
    let pool = sqlx::sqlite::SqlitePool::connect(&db).await.unwrap();

    let source_before = read_source(&pool, &meeting_id).await;
    let hash_before = source_hash(&source_before);
    println!("source before: {} rows, sha {hash_before}", source_before.len());

    let threshold: f64 =
        sqlx::query("SELECT speakerMergeThreshold FROM settings LIMIT 1")
            .fetch_optional(&pool)
            .await
            .unwrap()
            .map(|r| sqlx::Row::get::<f64, _>(&r, "speakerMergeThreshold"))
            .unwrap_or(0.40);
    let threshold_fp = (threshold as f32 * 65536.0) as u32;

    let registry: Arc<
        Mutex<Option<app_lib::audio::speaker::sherpa_adapter::CosineRegistryAdapter>>,
    > = Arc::new(Mutex::new(None));
    let t0 = std::time::Instant::now();
    let result =
        app_lib::audio::speaker::commands::run_speakers_reset_standalone(
            &pool,
            &meeting_id,
            threshold_fp,
            registry,
        )
        .await
        .expect("Speakers run failed");
    println!(
        "[{:.0}s] Speakers run: {} speakers, {} segments labeled, unmatched={:?}",
        t0.elapsed().as_secs_f64(),
        result.speaker_count,
        result.segments_labeled,
        result.unmatched_manual_names
    );

    // The headline invariant: the run did not touch the immutable source.
    let source_after = read_source(&pool, &meeting_id).await;
    assert_eq!(
        source_hash(&source_after),
        hash_before,
        "Speakers run mutated transcript_sources"
    );

    // The re-derived rendering must be garbage-free.
    #[derive(sqlx::FromRow)]
    struct RenderRow {
        transcript: String,
        audio_start_time: Option<f64>,
        audio_end_time: Option<f64>,
        speaker_label: Option<String>,
    }
    let rows = sqlx::query_as::<_, RenderRow>(
        "SELECT transcript, audio_start_time, audio_end_time, speaker_label \
         FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time, id",
    )
    .bind(&meeting_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let mut flagged = Vec::new();
    for r in &rows {
        let (s, e) = match (r.audio_start_time, r.audio_end_time) {
            (Some(s), Some(e)) => (s * 1000.0, e * 1000.0),
            _ => continue,
        };
        if app_lib::audio::hallucination::audit(&r.transcript, s, e).is_garbage {
            flagged.push(r.transcript.clone());
        }
    }
    let mut badges: std::collections::BTreeMap<String, i64> = Default::default();
    for r in &rows {
        *badges
            .entry(r.speaker_label.clone().unwrap_or_default())
            .or_default() += 1;
    }
    println!(
        "rendering after: {} rows, badges {badges:?}, flagged {}/{}",
        rows.len(),
        flagged.len(),
        rows.len()
    );
    for f in &flagged {
        println!("  STILL FLAGGED: {f}");
    }
    assert!(flagged.is_empty(), "re-derived rendering resurrected flagged rows");
    pool.close().await;
}
