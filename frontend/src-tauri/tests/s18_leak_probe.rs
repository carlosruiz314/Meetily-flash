//! S18 leak-diagnosis probe (diarization-render-fidelity 5.5 follow-up):
//! the cross-stream leak veto degraded the S18 repair window
//! [161.71-191.05] (RUN 3: "a stream decode duplicates the other stream's
//! words") — but that window's applied render is the ear-attested good
//! state. This probe re-separates the window and prints each stream's
//! decode VERBATIM TO TERMINAL ONLY (the bright line: env-gated terminal,
//! never a repo file) so the shared run's true shape can be read and the
//! veto bar corrected.
//!
//! MEETIFY_LIVE_DIAG=1 MEETIFY_GATE_WHISPER_MODEL=large-v3-turbo-q5_0 \
//!   cargo test --release --test s18_leak_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::ports::VoiceSeparationPort;
use app_lib::audio::speaker::run_assembly::{decode_stream_utterances, StreamUtterance};
use app_lib::audio::speaker::separation::ConvTasNetSeparator;

/// The S18 repair window (CENSUS-REPAIR line, both gate logs).
const SPAN: (f64, f64) = (161.71, 191.05);

#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test s18_leak_probe -- --ignored --nocapture"]
async fn probe_s18_stream_decodes() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let samples_path = std::path::PathBuf::from(std::env::var(
        "MEETIFY_SAMPLES_PATH",
    )
    .expect("MEETIFY_SAMPLES_PATH (path to the samples_16k.f32 cache)"));
    let bytes = std::fs::read(&samples_path).expect("samples cache");
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL")
        .unwrap_or_else(|_| "large-v3-turbo-q5_0".to_string());
    let whisper_dir = std::path::Path::new(&home).join("AppData/Roaming/com.meetily.ai/models");
    let mut engine = app_lib::whisper_engine::WhisperEngine::new_with_models_dir(Some(whisper_dir))
        .expect("engine");
    engine.discover_models().await.expect("discover");
    engine.load_model(&model).await.expect("load model");

    let separator = ConvTasNetSeparator::from_models_dir().expect("separation model");
    // The blocking decode is spawn_blocking-ONLY (tokio panics on an async
    // worker — the seam's pinned constraint).
    let print = tokio::task::spawn_blocking(move || {
        let streams = separator.separate(&samples, SPAN).expect("separate window");
        for (i, st) in streams.iter().enumerate() {
            let lang = "en";
            let decode = |s: &[f32]| -> Option<String> {
                let text = engine.transcribe_span_blocking(s.to_vec(), lang)?;
                let trimmed = text.trim();
                if !trimmed.chars().any(|c| c.is_alphanumeric()) {
                    return None;
                }
                let end_ms = s.len() as f64 / 16_000.0 * 1000.0;
                let report = app_lib::audio::hallucination::audit(trimmed, 0.0, end_ms);
                if report.is_garbage {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            };
            let utts: Vec<StreamUtterance> = decode_stream_utterances(&st.samples, &decode);
            eprintln!("=== STREAM {} ===", i);
            for u in &utts {
                let abs = |s: f64| {
                    let t = SPAN.0 + s;
                    format!("{}:{:04.1}", (t / 60.0) as i64, t % 60.0)
                };
                eprintln!(
                    "[{} -> {}] {}",
                    abs(u.offset_s.0),
                    abs(u.offset_s.1),
                    u.text.as_deref().unwrap_or("(abstained)")
                );
            }
        }
    })
    .await
    .expect("join");
    let _ = print;
    eprintln!("=== END (verbatim, terminal only — do not paste into repo files) ===");
}
