//! Arbitration diag: pyannote per-frame activity RUNS vs TitaNet run-level
//! embeddings (env-gated, read-only).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag -- --ignored --nocapture pyannote_runs_vs_embeddings
//!
//! pyannote_activity_diag produced the argmax speaker track (runs ≥0.3s) for
//! the banter and interjection ROIs. This test scores those RUN-LEVEL labels
//! against the only independent voice evidence we have: TitaNet embeddings
//! extracted over each WHOLE run (≥1s windows are in the stable regime, per
//! the window-length sweep — sub-1s voiceprints are noise, so runs <0.9s are
//! excluded from group stats and 0.9–1.5s runs are flagged marginal).
//!
//! Decisive outputs:
//!   1. within-pyannote-label vs across-pyannote-label mean cosine
//!   2. within-DB-label vs across-DB-label mean cosine (the current
//!      pipeline's grouping, same runs, apples-to-apples)
//!   3. the anchor run B=9.38–13.03s ("You look like you've aged like five
//!      years", ear-verified Cynthia, one voice): its mean cosine to the P1
//!      family vs the P2 family, and to the DB Sp0/Sp1 families.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use ndarray::{Array1, Array3};
use ort::execution_providers::CPUExecutionProvider;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::TensorRef;

const SAMPLE_RATE: usize = 16000;
const WINDOW_SAMPLES: usize = 160_000; // 10s
const STEP_SAMPLES: usize = 16_000; // 1s step
const FRAME_SHIFT_SECS: f64 = 270.0 / 16000.0; // ~16.875ms

/// Regions of interest: (start, end, label). Banter + the 46:58 interjection.
const ROIS: &[(f64, f64, &str)] = &[(0.0, 57.0, "banter"), (2783.0, 2872.0, "interjection")];
const ROI_BUFFER_SECS: f64 = 12.0;

const AUDIO: &str = "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MODELS_DIR: &str = ".meetily-models";

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

/// (name, start, end, pyannote label) — lifted verbatim from the measured
/// argmax track in pyannote_activity_diag (banter + interjection ROIs),
/// runs ≥0.9s only. pylabel: 0/1/2 as emitted, -1 unused here.
const RUNS: &[(&str, f64, f64, i8)] = &[
    // ---- banter ROI ----
    ("A_banter_7.3", 7.29, 8.57, 2),
    ("B_ANCHOR_9.4-13.0", 9.38, 13.03, 1), // "You look like you've aged like five years"
    ("C_banter_13.4", 13.42, 14.78, 0),    // conf 0.38 — suspicious run
    ("D_banter_17.3", 17.30, 20.30, 2),
    ("E_banter_21.0", 20.98, 22.11, 1),
    ("F_banter_23.5", 23.52, 24.49, 1),
    ("G_banter_27.3", 27.34, 30.00, 2),
    ("H_banter_30.0", 30.00, 32.16, 1),
    ("I_banter_36.2", 36.21, 38.64, 1),
    ("J_banter_43.0", 43.00, 49.01, 1),
    ("K_banter_50.0", 50.00, 52.85, 1),
    ("N_banter_42.0", 41.98, 43.00, 2),
    ("O_banter_34.7", 34.66, 36.21, 2),
    ("P_banter_49.0", 49.01, 50.00, 2),
    ("Q_banter_54.0", 54.00, 55.01, 1),
    // ---- interjection ROI ----
    ("IA_intj_2795.4", 2795.38, 2798.94, 2),
    ("IB_intj_2802.1", 2802.08, 2804.52, 1),
    ("IC_intj_2805.0", 2805.00, 2806.65, 1),
    ("IE_intj_2809.0", 2808.95, 2810.80, 1),
    ("IF_intj_2811.7", 2811.70, 2812.74, 1),
    ("IH_intj_2814.9", 2814.87, 2818.33, 1),
    ("II_intj_2818.9", 2818.88, 2820.13, 1),
    ("IJ_intj_2821.6", 2821.64, 2823.56, 2),
    ("IK_intj_2824.0", 2824.00, 2826.87, 2),
    ("IM_intj_2829.1", 2829.11, 2831.00, 1),
    ("IO_intj_2832.0", 2832.00, 2833.31, 1),
    ("IR_intj_2835.8", 2835.79, 2839.71, 1),
    ("IS_intj_2840.1", 2840.10, 2841.67, 1),
    ("IT_intj_2842.6", 2842.63, 2845.33, 1),
    ("IU_intj_2847.6", 2847.57, 2849.01, 1),
    ("IV_intj_2849.0", 2849.01, 2850.42, 2),
    ("IW_intj_2852.9", 2852.94, 2856.03, 2),
    ("IA2_intj_2861.6", 2861.56, 2863.86, 1),
    ("IB2_intj_2864.7", 2864.67, 2866.00, 1),
    ("IC2_intj_2866.0", 2866.00, 2867.00, 2),
    ("ID2_intj_2867.0", 2867.00, 2868.41, 1),
];

#[tokio::test]
#[ignore = "live GPU pass: MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag -- --ignored --nocapture pyannote_runs_vs_embeddings"]
async fn pyannote_runs_vs_embeddings() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();

    let emb_path = format!("{}/{}/{}", home(), MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename());
    let extractor = NemoEmbeddingExtractor::new(&emb_path).expect("extractor");

    // DB turn labels for cross-checking groupings.
    let pool = sqlx::SqlitePool::connect(&format!(
        "sqlite:{}/{}?mode=ro", home(), DB_PATH))
        .await
        .expect("db connect");
    let db_rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time, speaker_label FROM transcripts \
         WHERE meeting_id = ? ORDER BY audio_start_time ASC")
        .bind(MEETING_ID)
        .fetch_all(&pool)
        .await
        .expect("fetch transcripts");
    drop(pool);
    let db_turns: Vec<(f64, f64, String)> = db_rows
        .into_iter()
        .filter_map(|r| {
            let s: Option<f64> = sqlx::Row::get(&r, "audio_start_time");
            let e: Option<f64> = sqlx::Row::get(&r, "audio_end_time");
            let l: Option<String> = sqlx::Row::get(&r, "speaker_label");
            Some((s?, e?, l.unwrap_or_else(|| "?".into())))
        })
        .collect();

    // Embed every run.
    let mut embs: Vec<(&str, f64, f64, i8, String, Vec<f32>)> = Vec::new();
    for &(name, a, b, py) in RUNS {
        let i0 = (a * 16_000.0) as usize;
        let i1 = ((b * 16_000.0) as usize).min(samples.len());
        let Some(e) = extractor.extract_embedding(&samples[i0..i1], 16_000) else {
            eprintln!("RUN {}: extract returned None", name);
            continue;
        };
        // DB label = the turn overlapping the run's midpoint most.
        let mid = (a + b) / 2.0;
        let db_label = db_turns
            .iter()
            .filter(|(s, en, _)| mid >= *s && mid < *en)
            .map(|(_, _, l)| l.clone())
            .next()
            .unwrap_or_else(|| "?".into());
        embs.push((name, a, b, py, db_label, e));
    }

    let dur = |r: &(&str, f64, f64, i8, String, Vec<f32>)| r.2 - r.1;
    let reliable: Vec<_> = embs.iter().filter(|r| dur(r) >= 1.4).collect();
    eprintln!(
        "ARB: {} runs embedded, {} reliable (≥1.4s)",
        embs.len(),
        reliable.len()
    );

    // All-pairs matrix (reliable only), sorted by time.
    eprintln!("\n--- all-pairs cosine (reliable runs ≥1.4s) ---");
    eprintln!("py=P_yannote db=DB_label");
    for (i, r) in reliable.iter().enumerate() {
        eprintln!(
            "  {:22} py{} db{} dur={:4.1}",
            r.0, r.3, r.4, dur(r)
        );
        let _ = i;
    }

    let group_stats = |key: &dyn Fn(&(&str, f64, f64, i8, String, Vec<f32>)) -> String| {
        let mut groups: std::collections::HashMap<String, Vec<usize>> =
            std::collections::HashMap::new();
        for (i, r) in reliable.iter().enumerate() {
            groups.entry(key(r)).or_default().push(i);
        }
        let mut within = Vec::new();
        let mut across = Vec::new();
        for i in 0..reliable.len() {
            for j in (i + 1)..reliable.len() {
                let c = cosine(&reliable[i].5, &reliable[j].5);
                if reliable[i].0 == "B_ANCHOR_9.4-13.0" || reliable[j].0 == "B_ANCHOR_9.4-13.0" {
                    continue; // anchor scored separately
                }
                if key(reliable[i]) == key(reliable[j]) {
                    within.push(c);
                } else {
                    across.push(c);
                }
            }
        }
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len().max(1) as f32;
        let names: Vec<String> = {
            let mut v: Vec<String> = groups.keys().cloned().collect();
            v.sort();
            v
        };
        (mean(&within), within.len(), mean(&across), across.len(), names)
    };

    eprintln!("\n--- group separation (reliable runs, anchor excluded) ---");
    let (w, wn, a, an, _) = group_stats(&|r| format!("py{}", r.3));
    eprintln!(
        "pyannote labels: within={:.3} (n={})  across={:.3} (n={})  delta={:+.3}",
        w, wn, a, an, w - a
    );
    let (w, wn, a, an, _) = group_stats(&|r| r.4.clone());
    eprintln!(
        "DB pipeline labels: within={:.3} (n={})  across={:.3} (n={})  delta={:+.3}",
        w, wn, a, an, w - a
    );

    // Per-run mean similarity to each pyannote family (reliable members only).
    eprintln!("\n--- per-run affinity to pyannote families (reliable members) ---");
    for py_fam in [0i8, 1, 2] {
        let fam: Vec<&Vec<f32>> = reliable
            .iter()
            .filter(|r| r.3 == py_fam && r.0 != "B_ANCHOR_9.4-13.0")
            .map(|r| &r.5)
            .collect();
        if fam.is_empty() {
            continue;
        }
        let row: Vec<String> = reliable
            .iter()
            .filter(|r| r.3 != py_fam)
            .map(|r| {
                let m = fam.iter().map(|f| cosine(f, &r.5)).sum::<f32>() / fam.len() as f32;
                format!("{}={:.3}", r.0.split('_').next().unwrap(), m)
            })
            .collect();
        eprintln!("  vs py{} family (n={}): {}", py_fam, fam.len(), row.join(" "));
    }

    // Anchor verdict numbers.
    eprintln!("\n--- ANCHOR B=9.38-13.03s affinity ---");
    for (fam_name, pred) in [
        ("py1 family", 1i8),
        ("py2 family", 2i8),
        ("py0 family", 0i8),
    ] {
        let fam: Vec<&Vec<f32>> = reliable
            .iter()
            .filter(|r| r.3 == pred && r.0 != "B_ANCHOR_9.4-13.0")
            .map(|r| &r.5)
            .collect();
        if fam.is_empty() {
            continue;
        }
        let m = fam.iter().map(|f| cosine(f, &reliable.iter().find(|r| r.0 == "B_ANCHOR_9.4-13.0").unwrap().5)).sum::<f32>() / fam.len() as f32;
        eprintln!("  B vs {} (n={}): {:.3}", fam_name, fam.len(), m);
    }
    for (fam_name, db_lab) in [("DB Sp0", "Speaker 0"), ("DB Sp1", "Speaker 1"), ("DB Sp2", "Speaker 2")] {
        let fam: Vec<&Vec<f32>> = reliable
            .iter()
            .filter(|r| r.4 == db_lab && r.0 != "B_ANCHOR_9.4-13.0")
            .map(|r| &r.5)
            .collect();
        if fam.is_empty() {
            continue;
        }
        let m = fam.iter().map(|f| cosine(f, &reliable.iter().find(|r| r.0 == "B_ANCHOR_9.4-13.0").unwrap().5)).sum::<f32>() / fam.len() as f32;
        eprintln!("  B vs {} (n={}): {:.3}", fam_name, fam.len(), m);
    }

    // Marginal-run warnings: embeddings on <1.4s runs are not reliable evidence.
    let marginal: Vec<String> = embs
        .iter()
        .filter(|r| dur(r) < 1.4)
        .map(|r| format!("{} ({:.1}s)", r.0, dur(r)))
        .collect();
    if !marginal.is_empty() {
        eprintln!("\nARB: excluded from group stats (<1.4s): {}", marginal.join(", "));
    }
}

// ============================================================================
// Test 1: the argmax-track extraction that produced the RUNS table above.
// Re-run to regenerate: MEETIFY_LIVE_DIAG=1 cargo test --release
//   --test pyannote_activity_diag -- --ignored --nocapture pyannote_activity_as_primary_label
// ============================================================================

/// Powerset class → 3-speaker multilabel (pyannote-audio powerset.py).
/// 0 = no speech; 1–3 solo; 4–6 overlap pairs.
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

/// One frame's decoded probabilities: per-speaker mass (sum of exp(log_p) over
/// classes containing the speaker) + overlap-pair mass + silence mass.
#[derive(Clone, Copy, Default)]
struct FrameProbs {
    speaker: [f32; 3],
    overlap: f32, // classes 4+5+6
    silence: f32, // class 0
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
                    fp.overlap += p;
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

fn frame_time(idx: usize) -> f64 {
    idx as f64 * FRAME_SHIFT_SECS
}

/// A contiguous argmax run (a candidate turn).
#[derive(Clone, Copy)]
struct Run {
    start: f64,
    end: f64,
    speaker: i8, // 0..2, or -1 = silence
    mean_conf: f32,
}

/// Argmax label track over speech-dominant frames → runs ≥ min_dur_secs.
fn argmax_runs(frames: &[FrameProbs], from: usize, to: usize, min_dur_secs: f64) -> Vec<Run> {
    let mut raw: Vec<Run> = Vec::new();
    for i in from..to.min(frames.len()) {
        let fp = &frames[i];
        let speech = fp.speaker[0] + fp.speaker[1] + fp.speaker[2];
        let (label, conf) = if speech > 0.5 {
            let mut best = 0usize;
            for s in 1..3 {
                if fp.speaker[s] > fp.speaker[best] {
                    best = s;
                }
            }
            (best as i8, fp.speaker[best])
        } else {
            (-1, fp.silence)
        };
        match raw.last_mut() {
            Some(r) if r.speaker == label => {
                r.end = frame_time(i + 1);
                let n = ((r.end - r.start) / FRAME_SHIFT_SECS) as f32;
                r.mean_conf += (conf - r.mean_conf) / n.max(1.0);
            }
            _ => raw.push(Run {
                start: frame_time(i),
                end: frame_time(i + 1),
                speaker: label,
                mean_conf: conf,
            }),
        }
    }
    let min_dur = min_dur_secs;
    let mut merged: Vec<Run> = Vec::new();
    for r in raw {
        if r.end - r.start >= min_dur {
            if let Some(prev) = merged.last_mut() {
                if prev.speaker == r.speaker {
                    prev.end = r.end;
                    continue;
                }
            }
            merged.push(r);
        } else if let Some(prev) = merged.last_mut() {
            prev.end = r.end;
        }
    }
    let mut out: Vec<Run> = Vec::new();
    for r in merged {
        match out.last_mut() {
            Some(prev) if prev.speaker == r.speaker => prev.end = r.end,
            _ => out.push(r),
        }
    }
    out
}

#[tokio::test]
#[ignore = "live GPU pass: MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag -- --ignored --nocapture pyannote_activity_as_primary_label"]
async fn pyannote_activity_as_primary_label_cde5c264() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let model_path = format!("{}/.meetily-models/pyannote-segmentation.onnx", home());

    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("ACT: decoded {:.1}s", decoded.duration_seconds);

    // Session config = production pyannote_segmentation.rs parity.
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

    let total_windows_full = if samples.len() > WINDOW_SAMPLES {
        (samples.len() - WINDOW_SAMPLES) / STEP_SAMPLES + 1
    } else {
        1
    };
    let in_roi = |win_start_secs: f64| -> bool {
        let win_end = win_start_secs + WINDOW_SAMPLES as f64 / SAMPLE_RATE as f64;
        ROIS.iter()
            .any(|&(ws, we, _)| win_end >= ws - ROI_BUFFER_SECS && win_start_secs <= we + ROI_BUFFER_SECS)
    };

    let t0 = std::time::Instant::now();
    let mut frames: Vec<FrameProbs> = Vec::new();
    let mut windows_done = 0usize;
    for win_idx in 0..total_windows_full {
        let start = win_idx * STEP_SAMPLES;
        let win_start_secs = start as f64 / SAMPLE_RATE as f64;
        if !in_roi(win_start_secs) {
            continue;
        }
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
        let decoded_frames = decode_probs(slice, shape[1], shape[2]);

        let first_frame = (win_start_secs / FRAME_SHIFT_SECS).round() as usize;
        while frames.len() <= first_frame + decoded_frames.len() {
            frames.push(FrameProbs::default());
        }
        for (i, fp) in decoded_frames.into_iter().enumerate() {
            frames[first_frame + i] = fp;
        }
        windows_done += 1;
    }
    eprintln!(
        "ACT: {} ROI windows in {:.1}s, {} frames merged",
        windows_done,
        t0.elapsed().as_secs_f64(),
        frames.len()
    );

    for &(ws, we, label) in ROIS {
        let from = (ws / FRAME_SHIFT_SECS).round() as usize;
        let to = ((we / FRAME_SHIFT_SECS).round() as usize).min(frames.len());
        let runs = argmax_runs(&frames, from, to, 0.3);
        eprintln!("\n===== ROI: {} [{:.1}-{:.1}s] =====", label, ws, we);
        for r in &runs {
            let who = if r.speaker < 0 { "sil" } else { "P" };
            eprintln!(
                "  {:8.2}-{:8.2} ({:5.2}s) {}{} conf={:.2}",
                r.start, r.end, r.end - r.start, who, r.speaker, r.mean_conf
            );
        }
    }
    eprintln!("\nACT: done (runs above feed the RUNS table in pyannote_runs_vs_embeddings)");
}
