//! Voice-affinity probe for the 26–43s banter conflict (user adjudication
//! 2026-09-05: the ear wins — 32.0–38.0s is ONE voice, and the ≈30.0/≈32.65
//! changes are real).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --test banter_voice_probe -- --ignored --nocapture
//!
//! All-pairs TitaNet cosine between ear-anchored spans and the disputed
//! stretches. Decides WHERE the machine's error lives: embeddings confusing
//! the user with Ricardo, or a clustering mistake on correct embeddings.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use std::sync::Arc;

const MODELS_DIR: &str = ".meetily-models";
const AUDIO: &str =
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";

#[tokio::test]
#[ignore = "live probe: MEETIFY_LIVE_DIAG=1 cargo test --test banter_voice_probe -- --ignored --nocapture"]
async fn banter_voice_affinity() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = format!("{home}/{AUDIO}");
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();

    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{}/{}/{}",
        home,
        MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");

    // (name, start, end) — spans ≥1.5s embed their middle-12s (engine rule).
    let probes: Vec<(&str, f64, f64)> = vec![
        ("USER five-years  9.38-13.03 (ear)", 9.38, 13.03),
        ("USER yeah-sure  29.50-32.16 (ear)", 29.50, 32.16),
        ("CYND  yeah-that  13.42-14.78 (ear)", 13.42, 14.78),
        ("DISP  is-Ricardo 34.66-38.64 (ear:USER)", 34.66, 38.64),
        ("DISP  ping-in    41.98-52.85 (ear:USER)", 41.98, 52.85),
        ("RIC   stretch    2802.08-2820.13 (ear)", 2802.08, 2820.13),
        ("CYND  okay       39.00-39.93 (ear)", 39.00, 39.93),
    ];

    let sr = 16_000usize;
    let mut embs: Vec<(&str, Vec<f32>)> = Vec::new();
    for (name, a, b) in &probes {
        let dur = b - a;
        let (off_a, off_b) = if dur > 12.0 {
            let mid = (a + b) / 2.0;
            (mid - 6.0, mid + 6.0)
        } else {
            (*a, *b)
        };
        let i0 = (off_a * sr as f64) as usize;
        let i1 = ((off_b * sr as f64) as usize).min(samples.len());
        match extractor.extract_embedding(&samples[i0..i1], 16_000) {
            Some(e) => embs.push((name, e)),
            None => eprintln!("PROBE: {name} → no embedding"),
        }
    }

    fn cos(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na <= 0.0 || nb <= 0.0 { 0.0 } else { dot / (na * nb) }
    }

    eprintln!("PROBE: all-pairs TitaNet cosine");
    eprintln!(
        "{:>38} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "", "u-5yr", "u-yeah", "cynd", "disp34", "disp41", "ric"
    );
    let short = |n: &str| n.split_whitespace().next().unwrap_or(n).to_string();
    for (i, (ni, ei)) in embs.iter().enumerate() {
        let mut cells = Vec::new();
        for (j, (_, ej)) in embs.iter().enumerate() {
            cells.push(if i == j {
                "    —".to_string()
            } else {
                format!("{:.3}", cos(ei, ej))
            });
        }
        eprintln!("{:>38} {}", short(ni), cells.join(" "));
    }
    let _ = Arc::new(());
}
