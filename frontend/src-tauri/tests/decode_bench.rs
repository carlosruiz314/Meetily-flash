//! Times one 25 s slice through the three decode variants, to isolate why
//! the production batch decode (beam + auto-translate) ran ~25-30x realtime
//! on cde5c264 while the strict greedy repair decodes ran ~2 min/window in a
//! debug build. Reads the meeting folder's cached samples_16k.f32 (the
//! exact 16 kHz mono the batch lane decodes to).
//!
//!   cargo test --release --test decode_bench -- --ignored --nocapture 2>/dev/null
//!
//! (stderr carries whisper's C-level debug spew; redirect it away)

use app_lib::whisper_engine::WhisperEngine;
use std::path::PathBuf;
use std::time::Instant;

#[tokio::test]
#[ignore = "loads the real turbo model and decodes real audio; run explicitly"]
async fn decode_bench() {
    let folder = PathBuf::from(std::env::var("RT_FOLDER").expect("set RT_FOLDER"));
    let models_dir = PathBuf::from(
        std::env::var("RT_MODELS_DIR")
            .unwrap_or_else(|_| dirs::data_dir().unwrap().join("com.meetily.ai/models").to_string_lossy().to_string()),
    );

    let bytes = std::fs::read(folder.join("samples_16k.f32")).expect("read samples");
    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // A speech-dense slice: 1000-1025 s (the meeting's mid-section).
    let lo = 1000 * 16_000;
    let hi = 1025 * 16_000;
    let slice = samples[lo..hi.min(samples.len())].to_vec();
    println!("slice: {} samples (25 s @ 16 kHz)", slice.len());

    let engine = WhisperEngine::new_with_models_dir(Some(models_dir)).expect("engine init");
    engine.discover_models().await.expect("discover");
    engine.load_model("large-v3-turbo-q5_0").await.expect("load");

    // 1. Production batch decode, auto-translate (what the full run used).
    let t = Instant::now();
    let (text, _, _, _) = engine
        .transcribe_audio_with_confidence(slice.clone(), Some("auto-translate".into()), (lo / 16) as i64)
        .await
        .expect("decode 1");
    println!("PROD auto-translate: {:.1}s | {}", t.elapsed().as_secs_f32(), &text[..text.len().min(90)]);

    // 2. Production batch decode, pinned English (beam, no translate).
    let t = Instant::now();
    let (text, _, _, _) = engine
        .transcribe_audio_with_confidence(slice.clone(), Some("en".into()), (lo / 16) as i64)
        .await
        .expect("decode 2");
    println!("PROD en:             {:.1}s | {}", t.elapsed().as_secs_f32(), &text[..text.len().min(90)]);

    // 3. Strict greedy (the quarantine retry path).
    let t = Instant::now();
    let (text, _, _) = engine
        .transcribe_audio_strict(slice.clone(), Some("en".into()), (lo / 16) as i64)
        .await
        .expect("decode 3");
    println!("STRICT greedy en:    {:.1}s | {}", t.elapsed().as_secs_f32(), &text[..text.len().min(90)]);
}
