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

// ============================================================================
// Test 3: HYBRID ENGINE SIMULATION — the engine the arbitration numbers
// support: pyannote speech-runs are the turn units, ONE TitaNet embedding
// per run >=1.4s (reliable regime) carries all identity, sub-1.4s runs
// attach to their acoustically nearer neighbor, overlap mass is reported
// per turn. Full-meeting pass (production-parity geometry), no production
// code touched.
//
// MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag
//   -- --ignored --nocapture hybrid_engine_sim
// ============================================================================

const EMBED_MIN_SECS: f64 = 1.4;
const MERGE_THRESHOLD: f32 = 0.40; // production default
const SPEAKER_CAP: usize = 3; // cde5c264 meeting override

struct SimTurn {
    start: f64,
    end: f64,
    cluster: usize,
    overlap_frac: f32,
}

fn counts_merge(assign: &[usize], c: usize) -> usize {
    assign.iter().filter(|&&a| a == c).count()
}

#[tokio::test]
#[ignore = "live full-meeting pass (~15 min): MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag -- --ignored --nocapture hybrid_engine_sim"]
async fn hybrid_engine_sim_cde5c264() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let model_path = format!("{}/.meetily-models/pyannote-segmentation.onnx", home());
    let emb_path = format!(
        "{}/{}/{}",
        home(),
        MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename()
    );

    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("SIM: decoded {:.1}s", decoded.duration_seconds);

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

    // ---- Phase A: full-meeting per-frame activity (production geometry) ----
    let total_windows = if samples.len() > WINDOW_SAMPLES {
        (samples.len() - WINDOW_SAMPLES) / STEP_SAMPLES + 1
    } else {
        1
    };
    let t0 = std::time::Instant::now();
    let mut frames: Vec<FrameProbs> = Vec::new();
    for win_idx in 0..total_windows {
        let start = win_idx * STEP_SAMPLES;
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
        let first_frame = (win_idx as f64 / FRAME_SHIFT_SECS).round() as usize;
        while frames.len() <= first_frame + decoded_frames.len() {
            frames.push(FrameProbs::default());
        }
        for (i, fp) in decoded_frames.into_iter().enumerate() {
            frames[first_frame + i] = fp;
        }
        if win_idx % 500 == 0 {
            eprintln!(
                "SIM: window {}/{} ({:.0}s elapsed)",
                win_idx,
                total_windows,
                t0.elapsed().as_secs_f64()
            );
        }
    }
    eprintln!(
        "SIM: inference done: {} windows, {} frames, {:.0}s",
        total_windows,
        frames.len(),
        t0.elapsed().as_secs_f64()
    );

    // ---- Speech runs (pause-delimited sentence units) ----
    let all_runs = argmax_runs(&frames, 0, frames.len(), 0.3);
    let speech_runs: Vec<&Run> = all_runs.iter().filter(|r| r.speaker >= 0).collect();
    let total_speech: f64 = speech_runs.iter().map(|r| r.end - r.start).sum();
    let n_long = speech_runs
        .iter()
        .filter(|r| r.end - r.start >= EMBED_MIN_SECS)
        .count();
    let n_long_gt10 = speech_runs.iter().filter(|r| r.end - r.start >= 10.0).count();
    eprintln!(
        "SIM: {} speech runs ({} >=1.4s embeddable, {} >=10s need-split candidates), {:.0}s speech",
        speech_runs.len(),
        n_long,
        n_long_gt10,
        total_speech
    );

    // ---- Phase B: identity — one embedding per reliable run ----
    let extractor = app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor::new(&emb_path)
        .expect("extractor");
    let slice = |a: f64, b: f64| -> Vec<f32> {
        let i0 = (a * 16_000.0) as usize;
        let i1 = ((b * 16_000.0) as usize).min(samples.len());
        samples[i0..i1].to_vec()
    };

    // (run index in speech_runs, embedding) — ordered by run index.
    let mut reliable: Vec<(usize, Vec<f32>)> = Vec::new();
    for (ri, r) in speech_runs.iter().enumerate() {
        if r.end - r.start < EMBED_MIN_SECS {
            continue;
        }
        // Long runs: embed the middle 12s (mixed-run blends are surfaced via
        // the >=10s need-split count, not hidden).
        let (a, b) = if r.end - r.start > 12.0 {
            let mid = (r.start + r.end) / 2.0;
            (mid - 6.0, mid + 6.0)
        } else {
            (r.start, r.end)
        };
        if let Some(e) = extractor.extract_embedding(&slice(a, b), 16_000) {
            reliable.push((ri, e));
        }
    }
    eprintln!("SIM: {} reliable-run embeddings", reliable.len());

    // Greedy online clustering at a threshold.
    let cluster_at = |thr: f32| -> (Vec<usize>, Vec<Vec<f32>>) {
        let mut assign: Vec<usize> = Vec::new();
        let mut centroids: Vec<Vec<f32>> = Vec::new();
        let mut counts: Vec<usize> = Vec::new();
        for (_, e) in &reliable {
            let mut best: Option<(usize, f32)> = None;
            for (ci, c) in centroids.iter().enumerate() {
                let s = cosine(c, e);
                if s >= thr && best.map(|(_, bs)| s > bs).unwrap_or(true) {
                    best = Some((ci, s));
                }
            }
            match best {
                Some((ci, _)) => {
                    let n = counts[ci] as f32;
                    for (d, &v) in centroids[ci].iter_mut().zip(e.iter()) {
                        *d = (*d * n + v) / (n + 1.0);
                    }
                    counts[ci] += 1;
                    assign.push(ci);
                }
                None => {
                    centroids.push(e.clone());
                    counts.push(1);
                    assign.push(centroids.len() - 1);
                }
            }
        }
        (assign, centroids)
    };

    let (mut assign, mut centroids) = cluster_at(MERGE_THRESHOLD);
    let (assign55, _) = cluster_at(0.55);
    eprintln!(
        "SIM: greedy clusters at thr {:.2} = {}, at 0.55 = {}",
        MERGE_THRESHOLD,
        centroids.len(),
        assign55.iter().max().map(|m| m + 1).unwrap_or(0)
    );

    // Merge-to-cap: repeatedly fuse the closest centroid pair.
    while centroids.len() > SPEAKER_CAP {
        let mut best = (0usize, 1usize, -1.0f32);
        for i in 0..centroids.len() {
            for j in (i + 1)..centroids.len() {
                let s = cosine(&centroids[i], &centroids[j]);
                if s > best.2 {
                    best = (i, j, s);
                }
            }
        }
        let (i, j, _) = best;
        let (keep, gone) = if counts_merge(&assign, i) >= counts_merge(&assign, j) {
            (i, j)
        } else {
            (j, i)
        };
        for a in assign.iter_mut() {
            if *a == gone {
                *a = keep;
            }
        }
        centroids.remove(gone);
        // Indices above `gone` shifted down — remap before any centroid access.
        for a in assign.iter_mut() {
            if *a > gone {
                *a -= 1;
            }
        }
        // Recompute the kept centroid as the mean of its members.
        let dim = centroids[keep].len();
        let mut mean = vec![0.0f32; dim];
        let mut n = 0usize;
        for (k, ci) in assign.iter().enumerate() {
            if *ci == keep {
                n += 1;
                for d in 0..dim {
                    mean[d] += reliable[k].1[d];
                }
            }
        }
        for d in 0..dim {
            mean[d] /= n.max(1) as f32;
        }
        centroids[keep] = mean;
    }
    eprintln!("SIM: after merge-to-cap {} clusters", centroids.len());

    // Refine: reassign every reliable run to the nearest final centroid.
    for k in 0..assign.len() {
        let mut best = (0usize, -1.0f32);
        for (ci, c) in centroids.iter().enumerate() {
            let s = cosine(c, &reliable[k].1);
            if s > best.1 {
                best = (ci, s);
            }
        }
        assign[k] = best.0;
    }

    // ---- Phase C: turns — reliable runs keep their cluster; short runs attach
    // to the adjacent reliable run whose cluster centroid is nearest (noise-
    // regime embeddings are only ever compared to the two neighbors, never
    // trusted alone). Same-cluster neighbors separated only by absorbed
    // silence coalesce.
    let mut sim_turns: Vec<SimTurn> = Vec::new();
    for (ri, r) in speech_runs.iter().enumerate() {
        let cluster = match reliable.iter().position(|(k, _)| *k == ri) {
            Some(k) => assign[k],
            None => {
                // prev/next are RELIABLE POSITIONS (indices into `assign`),
                // not run indexes — reliable is sorted by run index, so a
                // scan gives the nearest neighbors on each side.
                let prev = (0..reliable.len()).rev().find(|&k| reliable[k].0 < ri);
                let next = (0..reliable.len()).find(|&k| reliable[k].0 > ri);
                let my_emb = extractor
                    .extract_embedding(&slice(r.start, r.end), 16_000)
                    .unwrap_or_else(|| vec![0.0; centroids[0].len()]);
                let dist = |k: usize| -> f32 {
                    centroids[assign[k]]
                        .iter()
                        .zip(my_emb.iter())
                        .map(|(c, v)| (c - v).powi(2))
                        .sum()
                };
                match (prev, next) {
                    (Some(p), Some(n)) => {
                        if dist(p) <= dist(n) {
                            assign[p]
                        } else {
                            assign[n]
                        }
                    }
                    (Some(p), None) => assign[p],
                    (None, Some(n)) => assign[n],
                    (None, None) => 0,
                }
            }
        };
        let f0 = (r.start / FRAME_SHIFT_SECS).round() as usize;
        let f1 = ((r.end / FRAME_SHIFT_SECS).round() as usize).min(frames.len());
        let n = (f1.saturating_sub(f0)).max(1);
        let ov = frames[f0..f1].iter().filter(|fp| fp.overlap > 0.25).count() as f32 / n as f32;
        sim_turns.push(SimTurn { start: r.start, end: r.end, cluster, overlap_frac: ov });
        if sim_turns.len() >= 2 {
            let len = sim_turns.len();
            let (a, b) = (sim_turns[len - 2].cluster, sim_turns[len - 1].cluster);
            if a == b {
                let start = sim_turns[len - 2].start;
                let end = sim_turns[len - 1].end;
                let ov = sim_turns[len - 2]
                    .overlap_frac
                    .max(sim_turns[len - 1].overlap_frac);
                sim_turns[len - 2] = SimTurn { start, end, cluster: a, overlap_frac: ov };
                sim_turns.pop();
            }
        }
    }
    eprintln!("SIM: {} hybrid turns total", sim_turns.len());

    // ---- DB turns for naming + text ----
    let pool = sqlx::SqlitePool::connect(&format!("sqlite:{}/{}?mode=ro", home(), DB_PATH))
        .await
        .expect("db connect");
    let db_rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time, speaker_label, transcript FROM transcripts \
         WHERE meeting_id = ? ORDER BY audio_start_time ASC",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("fetch transcripts");
    drop(pool);
    let db_turns: Vec<(f64, f64, String, String)> = db_rows
        .into_iter()
        .filter_map(|r| {
            let s: Option<f64> = sqlx::Row::get(&r, "audio_start_time");
            let e: Option<f64> = sqlx::Row::get(&r, "audio_end_time");
            let l: Option<String> = sqlx::Row::get(&r, "speaker_label");
            let t: String = sqlx::Row::get(&r, "transcript");
            Some((s?, e?, l.unwrap_or_else(|| "?".into()), t))
        })
        .collect();

    // Map hybrid cluster -> DB label by maximum mid-overlap (naming only).
    let mut votes: std::collections::HashMap<(usize, String), f64> = std::collections::HashMap::new();
    for t in &sim_turns {
        let mid = (t.start + t.end) / 2.0;
        if let Some((_, _, l, _)) = db_turns.iter().find(|(s, e, _, _)| mid >= *s && mid < *e) {
            *votes.entry((t.cluster, l.clone())).or_default() += t.end - t.start;
        }
    }
    let mut cluster_names: std::collections::HashMap<usize, String> = std::collections::HashMap::new();
    let mut cids: Vec<usize> = votes.keys().map(|(c, _)| *c).collect();
    cids.sort();
    for c in cids {
        let best = votes
            .iter()
            .filter(|((cc, _), _)| *cc == c)
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|((_, l), _)| l.clone())
            .unwrap_or_else(|| "?".into());
        cluster_names.insert(c, best);
    }

    // ---- Report: the two ear-truth ROIs ----
    for &(ws, we, label) in ROIS {
        eprintln!("\n===== SIM ROI: {} [{:.0}-{:.0}s] =====", label, ws, we);
        for t in sim_turns.iter().filter(|t| t.start < we && t.end > ws) {
            let text: String = db_turns
                .iter()
                .filter(|(s, e, _, _)| (s + e) / 2.0 >= t.start && (s + e) / 2.0 < t.end)
                .map(|(_, _, _, tx)| tx.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!(
                "  {:8.2}-{:8.2} C{}({}) ov={:.2} {}",
                t.start,
                t.end,
                t.cluster,
                cluster_names.get(&t.cluster).map(|s| s.as_str()).unwrap_or("?"),
                t.overlap_frac,
                &text.chars().take(90).collect::<String>()
            );
        }
    }

    // ---- Anchor acceptance ----
    let covering: Vec<&SimTurn> = sim_turns
        .iter()
        .filter(|t| t.start < 12.8 && t.end > 9.5)
        .collect();
    let boundary_near_13 = sim_turns.iter().filter(|t| t.start > 12.8 && t.start < 14.2).count();
    let boundary_at_1207 = sim_turns.iter().filter(|t| t.start > 11.5 && t.start < 12.7).count();
    eprintln!("\n===== SIM ANCHOR =====");
    eprintln!(
        "  turns covering 9.5-12.8s: {} (labels: {:?})",
        covering.len(),
        covering
            .iter()
            .map(|t| cluster_names.get(&t.cluster).map(|s| s.as_str()).unwrap_or("?"))
            .collect::<Vec<_>>()
    );
    eprintln!("  boundaries in 11.5-12.7s (pipeline flip point): {}", boundary_at_1207);
    eprintln!(
        "  boundaries in 12.8-14.2s (real change, missed by pipeline): {}",
        boundary_near_13
    );

    // ---- Whole-meeting agreement with DB labels (naming-level) ----
    let mut agree = 0usize;
    let mut total_mid = 0usize;
    for t in &sim_turns {
        let mid = (t.start + t.end) / 2.0;
        if let Some((_, _, l, _)) = db_turns.iter().find(|(s, e, _, _)| mid >= *s && mid < *e) {
            total_mid += 1;
            if cluster_names.get(&t.cluster).map(|s| s.as_str()) == Some(l.as_str()) {
                agree += 1;
            }
        }
    }
    eprintln!(
        "SIM: hybrid-vs-DB label agreement on turn midpoints: {}/{} ({:.0}%)",
        agree,
        total_mid,
        100.0 * agree as f64 / total_mid.max(1) as f64
    );
    let high_ov = sim_turns.iter().filter(|t| t.overlap_frac >= 0.2).count();
    eprintln!("SIM: turns with >=20% overlap frames: {}", high_ov);

    // ---- Dump ----
    let dump = serde_json::json!({
        "turns": sim_turns.iter().map(|t| serde_json::json!({
            "start": t.start, "end": t.end, "cluster": t.cluster,
            "db_label": cluster_names.get(&t.cluster),
            "overlap_frac": t.overlap_frac,
        })).collect::<Vec<_>>(),
        "n_speech_runs": speech_runs.len(),
        "n_reliable": reliable.len(),
    });
    let out_path = std::env::var("TEMP").unwrap() + "/cde5c264_hybrid_sim.json";
    std::fs::write(&out_path, serde_json::to_string_pretty(&dump).unwrap()).expect("write dump");
    eprintln!("SIM: dump at {}", out_path);
}

// ============================================================================
// Test 4: FLIP PROBE — is the 02:12-02:50 speaker flip (the user's example)
// a real voice change or a model error? Embeddings of sub-slices across the
// flip, compared against far-away reference runs of each cluster. Also:
// per-second pyannote overlap mass over the same window to check whether the
// "crosstalk 83%" flag is real signal or threshold artifact.
//
// MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag
//   -- --ignored --nocapture flip_probe
// ============================================================================

#[tokio::test]
#[ignore = "live pass: MEETIFY_LIVE_DIAG=1 cargo test --release --test pyannote_activity_diag -- --ignored --nocapture flip_probe"]
async fn flip_probe_cde5c264() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let model_path = format!("{}/.meetily-models/pyannote-segmentation.onnx", home());
    let emb_path = format!(
        "{}/{}/{}",
        home(),
        MODELS_DIR,
        app_lib::audio::speaker::model_download::embedding_filename()
    );

    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();

    let extractor = app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor::new(&emb_path)
        .expect("extractor");
    let slice = |a: f64, b: f64| -> Vec<f32> {
        let i0 = (a * 16_000.0) as usize;
        let i1 = ((b * 16_000.0) as usize).min(samples.len());
        samples[i0..i1].to_vec()
    };

    // (name, start, end) — the disputed window plus far-away references.
    // R1 = sim turn [02:12-02:39] labeled Sp0; R2 = [02:39-02:50] labeled Sp1.
    // "And I | was like" straddles the 159s boundary per the user (one voice).
    let windows: Vec<(&str, f64, f64)> = vec![
        ("R1_a", 133.0, 141.5),
        ("R1_b", 141.5, 150.0),
        ("R1_c", 150.0, 158.5),
        ("R1_tail_AndI", 155.0, 158.8),
        ("R2_head_wasLike", 159.3, 163.0),
        ("R2_b", 163.0, 166.5),
        ("R2_c", 166.5, 169.5),
        ("ref_Sp0_a", 58.5, 70.0),    // "Okay we can start..." (sim Sp0)
        ("ref_Sp0_b", 104.0, 127.0),  // "your image search..." (sim Sp0)
        ("ref_Sp1_a", 36.5, 52.0),    // "is Ricardo I don't know..." (sim Sp1)
        ("ref_Sp1_b", 69.6, 73.6),    // "there so we should be okay" (sim Sp1)
        ("ref_Sp2", 2803.0, 2819.5),  // "I can still analyze..." (sim Sp2)
    ];

    let mut embs: Vec<(&str, Vec<f32>)> = Vec::new();
    for (name, a, b) in &windows {
        match extractor.extract_embedding(&slice(*a, *b), 16_000) {
            Some(e) => embs.push((name, e)),
            None => eprintln!("FLIP: {} extract failed", name),
        }
    }

    eprintln!("\n--- all-pairs cosine: disputed window vs references ---");
    let mut header = String::from("                ");
    for (n, _, _) in &windows {
        header.push_str(&format!("{:>8}", n.chars().take(7).collect::<String>()));
    }
    eprintln!("{}", header);
    for (ni, ei) in embs.iter() {
        let mut row = format!("{:>16}", ni.chars().take(15).collect::<String>());
        for (_, ej) in embs.iter() {
            let dot: f32 = ei.iter().zip(ej).map(|(x, y)| x * y).sum();
            let na: f32 = ei.iter().map(|x| x * x).sum::<f32>().sqrt();
            let nb: f32 = ej.iter().map(|x| x * x).sum::<f32>().sqrt();
            let c = if na <= 0.0 || nb <= 0.0 { 0.0 } else { dot / (na * nb) };
            row.push_str(&format!("{:>8.3}", c));
        }
        eprintln!("{}", row);
    }

    // Mean cosine of each disputed slice to each reference family.
    let fam = |pred: &dyn Fn(&str) -> bool| -> Vec<Vec<f32>> {
        embs.iter()
            .filter(|(n, _)| pred(n))
            .map(|(_, e)| e.clone())
            .collect()
    };
    let mean_to = |e: &[f32], famv: &Vec<Vec<f32>>| -> f32 {
        if famv.is_empty() {
            return 0.0;
        }
        famv.iter()
            .map(|f| {
                let dot: f32 = f.iter().zip(e).map(|(x, y)| x * y).sum();
                let nf: f32 = f.iter().map(|x| x * x).sum::<f32>().sqrt();
                let ne: f32 = e.iter().map(|x| x * x).sum::<f32>().sqrt();
                if nf <= 0.0 || ne <= 0.0 { 0.0 } else { dot / (nf * ne) }
            })
            .sum::<f32>()
            / famv.len() as f32
    };
    let sp0 = fam(&|n| n.starts_with("ref_Sp0"));
    let sp1 = fam(&|n| n.starts_with("ref_Sp1"));
    let sp2 = fam(&|n| n.starts_with("ref_Sp2"));
    eprintln!("\n--- mean affinity per slice ---");
    for (n, e) in &embs {
        eprintln!(
            "  {:>16}  Sp0={:.3}  Sp1={:.3}  Sp2={:.3}",
            n,
            mean_to(e, &sp0),
            mean_to(e, &sp1),
            mean_to(e, &sp2)
        );
    }

    // Per-second pyannote activity over the disputed window: real overlap or
    // threshold artifact?
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

    const ROI_A: f64 = 118.0;
    const ROI_B: f64 = 184.0;
    let mut frames: Vec<FrameProbs> = Vec::new();
    let first_win = (ROI_A - 10.0).max(0.0) as usize;
    let last_win = (ROI_B as usize).min((samples.len() / SAMPLE_RATE) - 1);
    for w in first_win..=last_win {
        let start = w * STEP_SAMPLES;
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
        let slice_out = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());
        let decoded_frames = decode_probs(slice_out, shape[1], shape[2]);
        let first_frame = (w as f64 / FRAME_SHIFT_SECS).round() as usize;
        while frames.len() <= first_frame + decoded_frames.len() {
            frames.push(FrameProbs::default());
        }
        for (i, fp) in decoded_frames.into_iter().enumerate() {
            frames[first_frame + i] = fp;
        }
    }
    eprintln!("\n--- per-second pyannote activity 130-172s (spk masses / overlap) ---");
    let f0 = (130.0 / FRAME_SHIFT_SECS).round() as usize;
    let f1 = ((172.0 / FRAME_SHIFT_SECS).round() as usize).min(frames.len());
    let per = (1.0 / FRAME_SHIFT_SECS).round() as usize;
    let mut sec = 130;
    let mut i = f0;
    while i + per <= f1 {
        let chunk = &frames[i..i + per];
        let m = |get: &dyn Fn(&FrameProbs) -> f32| -> f32 {
            chunk.iter().map(get).sum::<f32>() / per as f32
        };
        eprintln!(
            "  {:>4}s  spk0={:.2} spk1={:.2} spk2={:.2}  ov>0.25: {:>2}/{:>2}  ov_mean={:.2}",
            sec,
            m(&|f: &FrameProbs| f.speaker[0]),
            m(&|f: &FrameProbs| f.speaker[1]),
            m(&|f: &FrameProbs| f.speaker[2]),
            chunk.iter().filter(|f| f.overlap > 0.25).count(),
            per,
            m(&|f: &FrameProbs| f.overlap),
        );
        i += per;
        sec += 1;
    }
}
