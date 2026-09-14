//! Agent-driven full Speakers run against the REAL app database and REAL
//! recording — no UI click. Drives the same production pipeline as the
//! Speakers button via `run_speakers_reset_standalone` (enumerate manual
//! labels → clear ALL → PreCleared re-run). Ignored by default; run
//! explicitly:
//!
//!   cargo test --test live_speakers_run -- --ignored --nocapture

use std::sync::{Arc, Mutex};

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";

fn app_db_path() -> std::path::PathBuf {
    dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("meeting_minutes.sqlite")
}

#[tokio::test]
#[ignore = "drives the REAL app DB + REAL recording; run explicitly with --ignored"]
async fn live_speakers_run_cde5c264() {
    // Surface the pipeline's log::warn!/info! stage lines (the test binary
    // has no logger otherwise — they'd be silently dropped).
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();

    let db = app_db_path();
    assert!(db.exists(), "app DB not found at {}", db.display());
    let pool = sqlx::sqlite::SqlitePool::connect(db.to_str().unwrap())
        .await
        .unwrap();

    // Same threshold source as AppState::sync_threshold_from_db.
    let threshold: f64 = sqlx::query("SELECT speakerMergeThreshold FROM settings LIMIT 1")
        .fetch_optional(&pool)
        .await
        .unwrap()
        .map(|r| sqlx::Row::get::<f64, _>(&r, "speakerMergeThreshold"))
        .unwrap_or(0.40);
    let threshold_fp = (threshold as f32 * 65536.0) as u32;
    eprintln!("merge threshold {:.2} (fp {})", threshold, threshold_fp);

    let registry: Arc<
        Mutex<Option<app_lib::audio::speaker::sherpa_adapter::CosineRegistryAdapter>>,
    > = Arc::new(Mutex::new(None));

    let t0 = std::time::Instant::now();
    let result = app_lib::audio::speaker::commands::run_speakers_reset_standalone(
        &pool,
        MEETING_ID,
        threshold_fp,
        registry,
    )
    .await
    .expect("diarization run failed");
    eprintln!(
        "RUN COMPLETE in {:.1}s: {} speakers, {} segments labeled, unmatched={:?}",
        t0.elapsed().as_secs_f64(),
        result.speaker_count,
        result.segments_labeled,
        result.unmatched_manual_names
    );

    // Immediate sample of the persisted shape the UI serves.
    let rows: Vec<(Option<f64>, Option<String>, String)> = sqlx::query_as(
        "SELECT audio_start_time, speaker_label, transcript \
         FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time LIMIT 14",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    eprintln!("--- first {} persisted rows ---", rows.len());
    for r in &rows {
        eprintln!(
            "[{:>7.2}] {:?} | {}",
            r.0.unwrap_or(0.0),
            r.1,
            r.2
        );
    }

    pool.close().await;
}
