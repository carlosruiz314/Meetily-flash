//! Closure-gap probe: what signal, if any, exists inside the pyannote-blind
//! 26.4–34.66s stretch for the missing ≈30.0s / ≈32.65s boundaries (S4/S5/S6)?
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan --test closure_gap_probe -- --ignored --nocapture
//!
//! Three independent signals, one audio decode:
//!   1. F0 trajectory (pure DSP): the region is male/female alternation; a
//!      sustained median-F0 step at 30.0/32.65 is a physical split signal.
//!   2. pyannote per-window identity votes: if windows straddling the region
//!      each decode ONE speaker but disagree on WHICH slot, the vote
//!      crossover localizes the change; if the slot is stable, this signal
//!      class is dead.
//!   3. TitaNet sliding-window trajectory: anchor affinities and
//!      adjacent-window cosine at sub-piece scale (the engine embeds only
//!      ≥1.5s pieces, so sub-piece voice changes are invisible to it today).
//!
//! Anchors are verbatim clip answers: USER 29.50–32.16 ("Yeah, sure, sure,
//! sure… for Paulina, right?" is the user), CYND 39.00–39.93 (Cynthia's
//! "okay" interjection), RIC 2802.08–2820.13 (clip 03/08, Ricardo).

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use ndarray::{Array1, Array3};
use ort::execution_providers::CPUExecutionProvider;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;

const AUDIO: &str =
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MODELS_DIR: &str = ".meetily-models";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const SAMPLE_RATE: usize = 16_000;
const WINDOW_SAMPLES: usize = 160_000; // 10s
const STEP_SAMPLES: usize = 16_000; // 1s step
const FRAME_SHIFT_SECS: f64 = 270.0 / 16_000.0; // ~16.875ms

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

/// Voiced F0 of one 40ms frame via normalized time-domain autocorrelation
/// (60–350 Hz). None = unvoiced/weak.
fn frame_f0(frame: &[f32], sr: usize) -> Option<f32> {
    let n = frame.len();
    let mean: f32 = frame.iter().sum::<f32>() / n as f32;
    let x: Vec<f32> = frame.iter().map(|&s| s - mean).collect();
    let e0: f32 = x.iter().map(|v| v * v).sum();
    if e0 < 1e-3 {
        return None;
    }
    let min_lag = (sr / 350).max(2);
    let max_lag = (sr / 60).min(n - 1);
    let mut best_r = 0.0f32;
    let mut best_lag = 0usize;
    for lag in min_lag..=max_lag {
        let mut dot = 0.0f32;
        let mut e1 = 0.0f32;
        for i in 0..(n - lag) {
            dot += x[i] * x[i + lag];
            e1 += x[i + lag] * x[i + lag];
        }
        if e1 <= 0.0 {
            continue;
        }
        let r = dot / (e0 * e1).sqrt();
        if r > best_r {
            best_r = r;
            best_lag = lag;
        }
    }
    if best_r < 0.5 {
        return None;
    }
    Some(sr as f32 / best_lag as f32)
}

/// Powerset class → 3-speaker multilabel (pyannote-audio powerset.py).
fn powerset_to_multilabel(class: usize) -> [bool; 3] {
    match class {
        0 => [false, false, false],
        1 => [true, false, false],
        2 => [false, true, false],
        3 => [false, false, true],
        4 => [true, true, false],
        5 => [true, false, true],
        6 => [false, true, true],
        _ => [false, false, false],
    }
}

#[derive(Clone, Copy, Default)]
struct FrameProbs {
    speaker: [f32; 3],
    silence: f32,
}

fn decode_probs(logits: &[f32], num_frames: usize, num_classes: usize) -> Vec<FrameProbs> {
    let mut out = Vec::with_capacity(num_frames);
    for frame in 0..num_frames {
        let row = &logits[frame * num_classes..(frame + 1) * num_classes];
        let mut fp = FrameProbs::default();
        for (class, &log_p) in row.iter().enumerate() {
            let p = log_p.exp();
            match class {
                0 => fp.silence += p,
                1 | 2 | 3 => fp.speaker[class - 1] += p,
                4 | 5 | 6 => {
                    let ml = powerset_to_multilabel(class);
                    for spk in 0..3 {
                        if ml[spk] {
                            fp.speaker[spk] += p;
                        }
                    }
                }
                _ => {}
            }
        }
        out.push(fp);
    }
    out
}

#[tokio::test]
#[ignore = "live probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test closure_gap_probe -- --ignored --nocapture s14_boundary_signals"]
async fn s14_boundary_signals() {
    // Adjudicate the S14 boundary: the engine's new trust-zone split lands at
    // 161.36; the fixture pin (from the committed flip probe) says 162.78.
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();

    // Anchors: USER 29.50-32.16, CYND 39.00-39.93 (same as closure_gap_signals).
    let emb_path = format!(
        "{}/{}/{}",
        home(),
        MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename()
    );
    let extractor = NemoEmbeddingExtractor::new(&emb_path).expect("extractor");
    let embed = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = (a * SAMPLE_RATE as f64) as usize;
        let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)
    };
    let anchors: Vec<(&str, Vec<f32>)> = vec![
        ("U", embed(29.50, 32.16).expect("user anchor")),
        ("C", embed(39.00, 39.93).expect("cynd anchor")),
    ];
    let aff = |e: &[f32]| -> String {
        anchors
            .iter()
            .map(|(n, c)| format!("{}={:.3}", n, cosine(c, e)))
            .collect::<Vec<_>>()
            .join(" ")
    };

    // F0 150-172s.
    eprintln!("--- F0 median per 0.25s block 150-172s ---");
    let block = 0.25f64;
    let frame_len = 640usize;
    let hop = 320usize;
    let mut t = 150.0f64;
    while t + block <= 172.0 {
        let i0 = (t * SAMPLE_RATE as f64) as usize;
        let i1 = (((t + block) * SAMPLE_RATE as f64) as usize).min(samples.len());
        let mut f0s: Vec<f32> = Vec::new();
        let mut j = 0usize;
        while j + frame_len <= i1.saturating_sub(i0) {
            if let Some(f) = frame_f0(&samples[i0 + j..i0 + j + frame_len], SAMPLE_RATE) {
                f0s.push(f);
            }
            j += hop;
        }
        if f0s.len() >= 3 {
            f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            eprintln!(
                "F0 {:6.2}-{:.2}  voiced {:>2}  med {:>5.0} Hz",
                t,
                t + block,
                f0s.len(),
                f0s[f0s.len() / 2]
            );
        } else {
            eprintln!("F0 {:6.2}-{:.2}  voiced {:>2}  (unvoiced)", t, t + block, f0s.len());
        }
        t += block;
    }

    // pyannote identity votes 152-170.
    let model_path = format!("{}/.meetily-models/pyannote-segmentation.onnx", home());
    let providers = vec![CPUExecutionProvider::default().build()];
    let session = Session::builder()
        .expect("builder")
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .expect("opt level")
        .with_execution_providers(providers)
        .expect("providers")
        .with_intra_threads(1)
        .expect("intra threads")
        .commit_from_file(&model_path)
        .expect("load pyannote");
    let input_name = session.inputs[0].name.to_string();
    let output_name = session.outputs[0].name.to_string();
    let mut session = session;

    eprintln!("\n--- pyannote identity votes 152-170 (mid-window frames [w+2,w+8)) ---");
    let probe_times = [
        157.0, 158.0, 159.0, 160.0, 160.8, 161.2, 161.6, 162.0, 162.4, 162.8, 163.2, 163.6,
        164.0, 165.0, 166.0, 168.0,
    ];
    for w in 152i64..=170 {
        let start = (w as usize) * STEP_SAMPLES;
        let end = (start + WINDOW_SAMPLES).min(samples.len());
        if end - start < SAMPLE_RATE {
            break;
        }
        let mut window = vec![0.0f32; WINDOW_SAMPLES];
        window[..end - start].copy_from_slice(&samples[start..end]);
        let input_3d: Array3<f32> = Array1::from(window)
            .into_shape_with_order([1, 1, WINDOW_SAMPLES])
            .unwrap();
        let tensor_ref = TensorRef::from_array_view(input_3d.view()).expect("tensor");
        let inputs = ort::inputs![input_name.as_str() => tensor_ref];
        let outputs = session.run(inputs).expect("forward");
        let out = outputs.get(output_name.as_str()).expect("output");
        let arr = out.try_extract_array::<f32>().expect("extract");
        let shape = arr.shape();
        let slice = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());
        let frames = decode_probs(slice, shape[1], shape[2]);

        let mut row = format!("VOTE w={:>3} ", w);
        for &pt in &probe_times {
            let local = pt - w as f64;
            if local < 2.0 || local >= 8.0 {
                continue;
            }
            let fidx = ((local / FRAME_SHIFT_SECS) as usize).min(frames.len() - 1);
            let fp = &frames[fidx];
            let speech = fp.speaker[0] + fp.speaker[1] + fp.speaker[2];
            if speech > 0.5 {
                let mut best = 0usize;
                for s in 1..3 {
                    if fp.speaker[s] > fp.speaker[best] {
                        best = s;
                    }
                }
                row.push_str(&format!("{:.1}:P{}({:.2}) ", pt, best + 1, fp.speaker[best]));
            } else {
                row.push_str(&format!("{:.1}:sil({:.2}) ", pt, fp.silence));
            }
        }
        eprintln!("{}", row);
    }

    // TitaNet 1.5s trajectory 150-172.
    eprintln!("\n--- TitaNet 1.5s windows hop 0.25, 150-172s ---");
    let mut prev: Option<(f64, Vec<f32>)> = None;
    let mut s = 150.0f64;
    while s + 1.5 <= 172.0 {
        if let Some(e) = embed(s, s + 1.5) {
            let adj = prev
                .as_ref()
                .filter(|(ps, _)| (s - ps).abs() < 1e-6)
                .map(|(_, pe)| format!(" adj={:.3}", cosine(pe, &e)))
                .unwrap_or_default();
            eprintln!("TRAJ {:.2}-{:.2}  {}{}", s, s + 1.5, aff(&e), adj);
            prev = Some((s, e));
        }
        s += 0.25;
    }
    eprintln!("\nS14: done");
}

#[tokio::test]
#[ignore = "live probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test closure_gap_probe -- --ignored --nocapture"]
async fn closure_gap_signals() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("GAP: decoded {:.1}s", decoded.duration_seconds);

    // ---- 1. F0 trajectory 24–40s (40ms frames / 20ms hop, 0.25s blocks) ----
    eprintln!("\n--- F0 median per 0.25s block (60-350Hz, r>=0.5 voiced) ---");
    let block = 0.25f64;
    let frame_len = 640usize; // 40ms
    let hop = 320usize; // 20ms
    let mut t = 24.0f64;
    while t + block <= 40.0 {
        let i0 = (t * SAMPLE_RATE as f64) as usize;
        let i1 = (((t + block) * SAMPLE_RATE as f64) as usize).min(samples.len());
        let mut f0s: Vec<f32> = Vec::new();
        let mut j = 0usize;
        while j + frame_len <= i1.saturating_sub(i0) {
            if let Some(f) = frame_f0(&samples[i0 + j..i0 + j + frame_len], SAMPLE_RATE) {
                f0s.push(f);
            }
            j += hop;
        }
        let voiced = f0s.len();
        if voiced >= 3 {
            f0s.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = f0s[voiced / 2];
            eprintln!(
                "F0 {:6.2}-{:.2}  voiced {:>2}/{:>2}  med {:>5.0} Hz",
                t,
                t + block,
                voiced,
                (block * SAMPLE_RATE as f64 / hop as f64) as usize,
                med
            );
        } else {
            eprintln!("F0 {:6.2}-{:.2}  voiced {:>2}  (unvoiced)", t, t + block, voiced);
        }
        t += block;
    }

    // ---- 2. pyannote per-window identity votes over 20–45s ----
    let model_path = format!("{}/.meetily-models/pyannote-segmentation.onnx", home());
    let providers = vec![CPUExecutionProvider::default().build()];
    let session = Session::builder()
        .expect("builder")
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .expect("opt level")
        .with_execution_providers(providers)
        .expect("providers")
        .with_intra_threads(1)
        .expect("intra threads")
        .commit_from_file(&model_path)
        .expect("load pyannote");
    let input_name = session.inputs[0].name.to_string();
    let output_name = session.outputs[0].name.to_string();
    let mut session = session;

    eprintln!("\n--- pyannote identity votes (mid-window frames only, [w+2,w+8)) ---");
    let probe_times = [
        27.5, 28.5, 29.5, 29.9, 30.5, 31.5, 32.3, 32.7, 33.3, 34.2, 35.0, 36.0, 37.5, 38.3,
    ];
    for w in 20i64..=44 {
        let start = (w as usize) * STEP_SAMPLES;
        let end = (start + WINDOW_SAMPLES).min(samples.len());
        if end - start < SAMPLE_RATE {
            break;
        }
        let mut window = vec![0.0f32; WINDOW_SAMPLES];
        window[..end - start].copy_from_slice(&samples[start..end]);
        let input_3d: Array3<f32> = Array1::from(window)
            .into_shape_with_order([1, 1, WINDOW_SAMPLES])
            .unwrap();
        let tensor_ref = TensorRef::from_array_view(input_3d.view()).expect("tensor");
        let inputs = ort::inputs![input_name.as_str() => tensor_ref];
        let outputs = session.run(inputs).expect("forward");
        let out = outputs.get(output_name.as_str()).expect("output");
        let arr = out.try_extract_array::<f32>().expect("extract");
        let shape = arr.shape();
        let slice = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());
        let frames = decode_probs(slice, shape[1], shape[2]);

        let mut row = format!("VOTE w={:>2} ", w);
        for &pt in &probe_times {
            let local = pt - w as f64;
            if local < 2.0 || local >= 8.0 {
                continue;
            }
            let fidx = ((local / FRAME_SHIFT_SECS) as usize).min(frames.len() - 1);
            let fp = &frames[fidx];
            let speech = fp.speaker[0] + fp.speaker[1] + fp.speaker[2];
            if speech > 0.5 {
                let mut best = 0usize;
                for s in 1..3 {
                    if fp.speaker[s] > fp.speaker[best] {
                        best = s;
                    }
                }
                row.push_str(&format!("{:.1}:P{}({:.2}) ", pt, best + 1, fp.speaker[best]));
            } else {
                row.push_str(&format!("{:.1}:sil({:.2}) ", pt, fp.silence));
            }
        }
        eprintln!("{}", row);
    }

    // ---- 3. TitaNet sliding-window trajectory ----
    let emb_path = format!(
        "{}/{}/{}",
        home(),
        MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename()
    );
    let extractor = NemoEmbeddingExtractor::new(&emb_path).expect("extractor");
    let embed = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = (a * SAMPLE_RATE as f64) as usize;
        let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)
    };

    let anchors: Vec<(&str, Vec<f32>)> = vec![
        ("U", embed(29.50, 32.16).expect("user anchor")),
        ("C", embed(39.00, 39.93).expect("cynd anchor")),
        ("R", embed(2802.08, 2820.13).expect("ric anchor")),
    ];
    let aff = |e: &[f32]| -> String {
        anchors
            .iter()
            .map(|(n, c)| format!("{}={:.3}", n, cosine(c, e)))
            .collect::<Vec<_>>()
            .join(" ")
    };

    eprintln!("\n--- TitaNet 1.5s windows hop 0.25 (stable regime) ---");
    let mut prev: Option<(f64, f64, Vec<f32>)> = None;
    let mut s = 24.5f64;
    while s + 1.5 <= 39.2 {
        if let Some(e) = embed(s, s + 1.5) {
            let adj = prev
                .as_ref()
                .filter(|(ps, _, _)| (s - ps).abs() < 1e-6)
                .map(|(_, _, pe)| format!(" adj={:.3}", cosine(pe, &e)))
                .unwrap_or_default();
            eprintln!("TRAJ {:.2}-{:.2}  {}{}", s, s + 1.5, aff(&e), adj);
            prev = Some((s, s + 1.5, e));
        }
        s += 0.25;
    }

    eprintln!("\n--- TitaNet 0.9s windows hop 0.1 around the two boundaries (marginal regime, relative read) ---");
    prev = None;
    let mut s = 29.0f64;
    while s + 0.9 <= 33.6 {
        if let Some(e) = embed(s, s + 0.9) {
            let adj = prev
                .as_ref()
                .filter(|(ps, _, _)| (s - ps).abs() < 1e-6)
                .map(|(_, _, pe)| format!(" adj={:.3}", cosine(pe, &e)))
                .unwrap_or_default();
            eprintln!("FINE {:.2}-{:.2}  {}{}", s, s + 0.9, aff(&e), adj);
            prev = Some((s, s + 1.5, e));
        }
        s += 0.1;
    }

    eprintln!("\n--- TitaNet point spans ---");
    let spans: Vec<(&str, f64, f64)> = vec![
        ("pre-flip frag 25.31-26.43", 25.31, 26.43),
        ("mixed piece 27.34-32.16", 27.34, 32.16),
        ("head 27.34-30.00", 27.34, 30.00),
        ("tail 30.00-32.16", 30.00, 32.16),
        ("mystery 32.16-34.66", 32.16, 34.66),
        ("yeah 32.60-33.00", 32.60, 33.00),
        ("disp-a 34.66-36.21", 34.66, 36.21),
        ("disp-b 36.21-38.64", 36.21, 38.64),
    ];
    for (name, a, b) in &spans {
        match embed(*a, *b) {
            Some(e) => eprintln!("SPAN {:<26} {}", name, aff(&e)),
            None => eprintln!("SPAN {:<26} (no embedding)", name),
        }
    }

    // ---- 4. transcript rows 20–45s (sentence structure / invariant risk) ----
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}/{}?mode=ro", home(), DB_PATH))
        .await
        .expect("db connect");
    let rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time, speaker_label, transcript FROM transcripts \
         WHERE meeting_id = ? AND audio_start_time >= 20.0 AND audio_start_time < 45.0 \
         ORDER BY audio_start_time ASC",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("fetch transcripts");
    drop(pool);
    eprintln!("\n--- transcript rows 20-45s ---");
    for r in &rows {
        let s: Option<f64> = sqlx::Row::get(r, "audio_start_time");
        let en: Option<f64> = sqlx::Row::get(r, "audio_end_time");
        let l: Option<String> = sqlx::Row::get(r, "speaker_label");
        let tx: Option<String> = sqlx::Row::get(r, "transcript");
        eprintln!(
            "ROW {:>6.2}-{:>6.2} {} {}",
            s.unwrap_or_default(),
            en.unwrap_or_default(),
            l.unwrap_or_else(|| "?".into()),
            tx.unwrap_or_default()
        );
    }
    eprintln!("\nGAP: done");
}
