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
    // The harness bypasses app startup — run the same migrations the app
    // would (the persist writes schema-dependent columns; an unmigrated DB
    // aborts the whole 80-minute run at persist time).
    sqlx::migrate!("./migrations").run(&pool).await.expect("run migrations");

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

    // Overlap-stream synthesis (overlap-stream-retranscription 3.4): the
    // seam reads the WHISPER_ENGINE static and the language preference —
    // both default to degrade in a fresh process. Populate them exactly as
    // the app does at startup: init the engine, load the meeting's model,
    // pin a CONCRETE language (automatic states degrade by design — never
    // auto-translate).
    // Same wiring as the app's startup (set_models_directory + whisper_init)
    // — the harness has no AppHandle, so the static is populated directly
    // with the production model store.
    let models_dir = dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("models");
    let engine = std::sync::Arc::new(
        app_lib::whisper_engine::whisper_engine::WhisperEngine::new_with_models_dir(Some(
            models_dir,
        ))
        .expect("engine ctor"),
    );
    engine.discover_models().await.expect("discover models");
    engine
        .load_model("large-v3-turbo-q5_0")
        .await
        .expect("load the meeting's whisper model");
    *app_lib::whisper_engine::commands::WHISPER_ENGINE.lock().unwrap() = Some(engine.clone());
    app_lib::set_language_preference_internal("en");
    eprintln!("whisper engine loaded, language pinned for the synthesis seam");

    // transcript_sources is immutable across the run (task 3.4): hash the
    // source table before and compare after.
    let sources_before: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, transcript FROM transcript_sources WHERE meeting_id = ? ORDER BY id",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .unwrap();

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

    // DB-layer verification (terminal-only rule): the S16 window carries the
    // per-voice synth pair; transcript_sources is untouched.
    let sources_after: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, transcript FROM transcript_sources WHERE meeting_id = ? ORDER BY id",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(sources_after, sources_before, "transcript_sources must be byte-identical across the run");

    let s16_rows: Vec<(Option<i64>, Option<String>, String)> = sqlx::query_as(
        "SELECT synth_atom, speaker_label, transcript FROM transcripts          WHERE meeting_id = ? AND audio_start_time >= 1054.0 AND audio_end_time <= 1059.0          ORDER BY audio_start_time, audio_end_time",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    eprintln!("--- S16 window rows (1054-1059s) ---");
    for r in &s16_rows {
        eprintln!("synth={:?} badge={:?} | {}", r.0, r.1, r.2);
    }
    let synth_count = s16_rows.iter().filter(|r| r.0 == Some(1)).count();
    eprintln!(
        "VERIFY: S16 window holds {synth_count} synth row(s) of {} total",
        s16_rows.len()
    );

    pool.close().await;
}
