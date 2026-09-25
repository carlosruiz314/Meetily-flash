//! Throwaway diagnostic: replay the engine from caches with sub-turn debug
//! on; print the turn list around a window of interest.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --release --test subturn_debug_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::{FrameMassesOutput, FRAME_SHIFT};
use app_lib::audio::speaker::run_engine;
use sqlx::Row;

const AUDIO: &str =
    "Music/local-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const MODELS_DIR: &str = ".meetily-models";
const MEETING_CAP: usize = 3;
const MERGE_THRESHOLD: f32 = 0.65;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test subturn_debug_probe -- --ignored --nocapture"]
async fn turns_around_window() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let lo: f64 = std::env::var("SUBTURN_WINDOW_LO")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(35.0);
    let hi: f64 = std::env::var("SUBTURN_WINDOW_HI")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50.0);

    let audio_path = format!("{}/{AUDIO}", home());
    let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
    let bytes = std::fs::read(dir.join("samples_16k.f32")).expect("samples cache");
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let cache_path = dir.join("gate_frame_masses.json");
    let models_dir = format!("{}/{MODELS_DIR}", home());
    let prov = app_lib::audio::speaker::pyannote_segmentation::cache_provenance(
        std::path::Path::new(&format!("{models_dir}/pyannote-segmentation.onnx")),
    )
    .expect("provenance");
    let fm = FrameMassesOutput::load(&cache_path, &prov).expect("frame-mass cache");

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(format!("{}/{}", home(), DB_PATH)),
        )
        .await
        .expect("db connect");
    let gate_rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time FROM transcript_sources WHERE meeting_id = ?",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("transcript_sources");
    let text_spans: Vec<(f64, f64)> = gate_rows
        .iter()
        .map(|r| {
            let s: f64 = r.get("audio_start_time");
            let e: f64 = r.get("audio_end_time");
            (s, e)
        })
        .collect();
    let references =
        app_lib::database::repositories::speaker::SpeakerRepository::list_enrollment_refs(&pool)
            .await
            .expect("refs");
    drop(pool);

    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{models_dir}/{}",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");

    let out = run_engine::derive_turns_from_masses(
        &fm,
        &extractor,
        &samples,
        &text_spans,
        MERGE_THRESHOLD,
        MEETING_CAP,
        &references,
    )
    .expect("engine run");
    eprintln!(
        "text_spans from transcript_sources: {} (fallback: none)",
        text_spans.len()
    );
    eprintln!("===== turns in [{lo}, {hi}] =====");
    for t in &out.turns {
        if t.end_seconds > lo && t.start_seconds < hi {
            eprintln!(
                "TURN [{:9.3},{:9.3}] sp{} cont={} lowc={}",
                t.start_seconds, t.end_seconds, t.speaker_id, t.continues_previous, t.low_confidence
            );
        }
    }
    eprintln!("total turns: {} (frame shift {FRAME_SHIFT})", out.turns.len());
}
