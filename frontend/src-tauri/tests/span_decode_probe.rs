//! Ear-truth repair probe: decode arbitrary spans of the cde5c264 samples
//! cache and print the text. Terminal-only output (verbatim text is printed
//! ONLY when MEETIFY_RENDER_PRINT is set; otherwise word counts + a sha256).
//!
//! Purpose (ear round 2026-10-01): the S18 window (179-199.5s) renders a
//! phrase-loop fabrication while the RMS profile shows real, unaccounted
//! speech bursts. This probe decodes named sub-spans so the render-repair
//! design can be checked against the attested truth WITHOUT a full run.
//!
//! Env:
//!   MEETIFY_LIVE_DIAG=1                       (gate for any model load)
//!   MEETIFY_SPAN_PROBE="187.3-190.0,183.8-186.0"  (required; seconds)
//!   MEETIFY_GATE_WHISPER_MODEL                (default large-v3-turbo-q5_0)
//!   MEETIFY_GATE_LANG                         (default en)
//!   MEETIFY_RENDER_PRINT                      (verbatim to terminal)

use app_lib::audio::speaker::ports::VoiceSeparationPort;
use app_lib::whisper_engine::whisper_engine::WhisperEngine;

const SR: u32 = 16_000;

fn samples_cache() -> std::path::PathBuf {
    let meeting = std::env::var("MEETIFY_MEETING_DIR")
        .expect("MEETIFY_MEETING_DIR (recording dir name)");
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let music = std::path::Path::new(&home).join("Music");
    for entry in std::fs::read_dir(&music).expect("read Music") {
        let p = entry.expect("dir entry").path();
        if p.is_dir() && p.join(meeting).join("samples_16k.f32").exists() {
            return p.join(meeting).join("samples_16k.f32");
        }
    }
    if let Ok(dir) = std::env::var("MEETIFY_RECORDINGS_DIR") {
        let p = std::path::Path::new(&dir).join(meeting).join("samples_16k.f32");
        if p.exists() {
            return p;
        }
    }
    panic!("no samples_16k.f32 cache found for {meeting}");
}

fn parse_spans(spec: &str) -> Vec<(f64, f64)> {
    spec.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            let (a, b) = s
                .split_once('-')
                .unwrap_or_else(|| panic!("span {s:?} must be START-END"));
            let (a, b) = (a.trim().parse::<f64>().expect("start"), b.trim().parse::<f64>().expect("end"));
            assert!(b > a, "span {s:?} end <= start");
            (a, b)
        })
        .collect()
}

#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --test span_decode_probe -- --ignored --nocapture"]
async fn decode_named_spans() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let spec = std::env::var("MEETIFY_SPAN_PROBE")
        .expect("MEETIFY_SPAN_PROBE must list spans as START-END,START-END (seconds)");
    let plain: Vec<&str> = spec
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("sep:"))
        .collect();
    if plain.is_empty() {
        eprintln!("no plain spans (only sep:?) — decode probe idle");
        return;
    }
    let spans = parse_spans(&plain.join(","));
    let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL")
        .unwrap_or_else(|_| "large-v3-turbo-q5_0".to_string());
    let lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| "en".to_string());

    let models_dir = dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("models");
    let mut engine = WhisperEngine::new_with_models_dir(Some(models_dir)).expect("engine ctor");
    engine.discover_models().await.expect("discover models");
    engine.load_model(&model).await.expect("load model");

    let cache = samples_cache();
    let raw = std::fs::read(&cache).expect("read samples cache");
    let all: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // The blocking decode is spawn_blocking-ONLY (tokio panics on a
    // runtime worker — the design's pinned constraint).
    let out = tokio::task::spawn_blocking(move || {
        let mut texts = Vec::new();
        for (a, b) in &spans {
            let lo = (*a * SR as f64) as usize;
            let hi = ((*b * SR as f64) as usize).min(all.len());
            let slice = all[lo..hi].to_vec();
            let text = engine
                .transcribe_span_blocking(slice, &lang)
                .unwrap_or_default();
            texts.push((*a, *b, text));
        }
        texts
    })
    .await
    .expect("decode join");

    for (a, b, text) in out {
        let words = text.split_whitespace().count();
        let print_verbatim = std::env::var("MEETIFY_RENDER_PRINT").is_ok();
        let shown = if print_verbatim {
            text.clone()
        } else {
            use sha2::{Digest, Sha256};
            format!("<{} words, sha {:x}>", words, Sha256::digest(text.as_bytes()))
        };
        eprintln!("SPAN [{a:.2}-{b:.2}] {words} words: {shown}");
    }
}

/// Separation probe: run Conv-TasNet over the named window and decode BOTH
/// streams — the go/no-go read for routing phrase-loop repair through the
/// synthesis pipeline. Spans prefixed "sep:" (e.g. "sep:183.15-194.24").
#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 + MEETIFY_SPAN_PROBE with sep: spans"]
async fn separate_named_spans() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let spec = std::env::var("MEETIFY_SPAN_PROBE").expect("MEETIFY_SPAN_PROBE");
    let sep_spec: Vec<(f64, f64)> = spec
        .split(',')
        .filter_map(|s| s.trim().strip_prefix("sep:"))
        .map(|s| {
            let (a, b) = s.split_once('-').expect("sep span START-END");
            (
                a.trim().parse::<f64>().expect("start"),
                b.trim().parse::<f64>().expect("end"),
            )
        })
        .collect();
    if sep_spec.is_empty() {
        eprintln!("no sep: spans — separation probe idle");
        return;
    }
    let model = std::env::var("MEETIFY_GATE_WHISPER_MODEL")
        .unwrap_or_else(|_| "large-v3-turbo-q5_0".to_string());
    let lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| "en".to_string());
    let models_dir = dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("models");
    let mut engine = WhisperEngine::new_with_models_dir(Some(models_dir)).expect("engine ctor");
    engine.discover_models().await.expect("discover models");
    engine.load_model(&model).await.expect("load model");
    let separator = app_lib::audio::speaker::separation::ConvTasNetSeparator::from_models_dir()
        .expect("separator (ONNX models in ~/.meetily-models)");

    let cache = samples_cache();
    let raw = std::fs::read(&cache).expect("read samples cache");
    let all: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let out = tokio::task::spawn_blocking(move || {
        let mut texts = Vec::new();
        for (a, b) in &sep_spec {
            match separator.separate(&all, (*a, *b)) {
                Ok(sep) => {
                    for (i, st) in sep.iter().enumerate() {
                        let ratio = st.pre_rms_ratio;
                        let text = engine
                            .transcribe_span_blocking(st.samples.clone(), &lang)
                            .unwrap_or_default();
                        texts.push((*a, *b, i, ratio, text));
                    }
                }
                Err(e) => eprintln!("SEP [{a:.2}-{b:.2}] separation failed: {e}"),
            }
        }
        texts
    })
    .await
    .expect("sep decode join");

    let print_verbatim = std::env::var("MEETIFY_RENDER_PRINT").is_ok();
    for (a, b, stream, ratio, text) in out {
        let words = text.split_whitespace().count();
        let shown = if print_verbatim {
            text
        } else {
            use sha2::{Digest, Sha256};
            format!("<{} words, sha {:x}>", words, Sha256::digest(text.as_bytes()))
        };
        eprintln!("SEP [{a:.2}-{b:.2}] stream{stream} rms_ratio={ratio:.3} {words} words: {shown}");
    }
}
