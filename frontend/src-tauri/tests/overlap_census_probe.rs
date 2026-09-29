//! overlap-stream-retranscription task 0.1: dry-run census (token-only).
//!
//! Blocking measurement before any implementation: does the attested S16
//! crosstalk window (1055.5–1057.5s) actually FIRE the production overlap
//! trigger (`OVERLAP_TRIGGER_MASS` = 0.5, sustained ≥ 0.4s)? The prepass
//! stop-gate scored S16 by running the separator directly on attested
//! coordinates — it never went through the trigger — so nothing yet proves
//! S16 is among the spans the pipeline would separate.
//!
//! Census over EVERY trigger-fired span: walls, trigger mass, per-stream
//! pre-normalization RMS ratio (normalization amplifies near-silence into
//! the hallucination regime), span-level stream identity (best/second
//! cosine, margin, decisive bar) including REJECTED streams — the
//! near-miss distribution the threshold-retuning protocol needs. Atom-level
//! votes need transcript token walls (gate-extension work); this is the
//! span-level identity read.
//!
//! Every output line is TOKEN-ONLY: numbers plus userA/userB/userC. No
//! render change, no persistence, nothing meeting-derived beyond these
//! numbers.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::{
    cache_provenance, FrameMassesOutput, PyannoteSegmentation, FRAME_SHIFT,
};
use app_lib::audio::speaker::run_assembly::{cosine, SUBTURN_VOTE_MARGIN};
use app_lib::audio::speaker::run_engine::{overlap_spans, OVERLAP_TRIGGER_MASS};
use app_lib::audio::speaker::separation::conv_tasnet::SEPARATION_CONTEXT_SECS;
use ndarray::Array2;
use ort::execution_providers::CPUExecutionProvider;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::TensorRef;

const SR: f64 = 16_000.0;
/// The attested crosstalk exchange (S16) — the window the whole change
/// exists to fix.
const SPAN_S16: (f64, f64) = (1055.5, 1057.5);
/// Profile window around S16 (a little wider than the exchange).
const S16_PROFILE: (f64, f64) = (1054.5, 1058.5);

// Solo reference windows (fixture-pinned single-voice rows; same as the
// quality probe).
const REF_USERA: (f64, f64) = (46.0, 50.0);
const REF_USERB: (f64, f64) = (1196.2, 1199.5);
const REF_USERC: (f64, f64) = (1201.0, 1210.0);

fn resolve_audio() -> std::path::PathBuf {
    let meeting = std::env::var("MEETIFY_MEETING_DIR")
        .expect("MEETIFY_MEETING_DIR (recording dir name)");
    if let Ok(dir) = std::env::var("MEETIFY_RECORDINGS_DIR") {
        return std::path::Path::new(&dir).join(meeting).join("audio.mp4");
    }
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let music = std::path::Path::new(&home).join("Music");
    for entry in std::fs::read_dir(&music).expect("read Music") {
        let p = entry.expect("dir entry").path();
        if p.is_dir() && p.join(meeting).join("audio.mp4").exists() {
            return p.join(meeting).join("audio.mp4");
        }
    }
    panic!("no *-recordings folder with {meeting} under {}", music.display());
}

fn load_samples(audio_path: &std::path::Path) -> Vec<f32> {
    use sha2::{Digest, Sha256};
    let dir = audio_path.parent().unwrap().to_path_buf();
    let cache = dir.join("samples_16k.f32");
    let meta = dir.join("samples_16k.meta.json");
    let source_sha = {
        let mut h = Sha256::new();
        let mut file = std::fs::File::open(audio_path).expect("open source audio");
        std::io::copy(&mut file, &mut h).expect("hash source audio");
        format!("{:x}", h.finalize())
    };
    if cache.exists() && meta.exists() {
        let meta_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&meta).expect("samples meta"))
                .expect("parse samples meta");
        if let Some(sha) = meta_json.get("audio_sha256").and_then(|v| v.as_str()) {
            if sha == source_sha {
                let bytes = std::fs::read(&cache).expect("read samples cache");
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                eprintln!(
                    "CENSUS: samples from cache ({:.1}s, provenance OK)",
                    samples.len() as f64 / SR
                );
                return samples;
            }
        }
        eprintln!("CENSUS: samples cache provenance mismatch — re-decoding");
    }
    let decoded = app_lib::audio::decoder::decode_audio_file(audio_path).expect("decode audio");
    let s = decoded.to_whisper_format();
    let bytes: Vec<u8> = s.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&cache, &bytes).expect("write samples cache");
    let meta_json = serde_json::json!({ "samples": s.len(), "audio_sha256": source_sha });
    std::fs::write(&meta, meta_json.to_string()).expect("write samples meta");
    eprintln!("CENSUS: decoded and cached {:.1}s", decoded.duration_seconds);
    s
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

/// `run_engine::overlap_spans` with the dials as parameters (same run
/// semantics) — the sweep needs span counts at candidate settings without
/// touching production constants.
fn overlap_spans_at(
    frames: &[app_lib::audio::speaker::pyannote_segmentation::FrameMasses],
    frame_shift: f64,
    mass_bar: f32,
    min_span_secs: f64,
) -> Vec<(f64, f64)> {
    let mut spans = Vec::new();
    let mut run_start: Option<usize> = None;
    for (i, f) in frames.iter().enumerate() {
        if f.overlap >= mass_bar {
            if run_start.is_none() {
                run_start = Some(i);
            }
        } else if let Some(s) = run_start {
            if (i - s) as f64 * frame_shift >= min_span_secs {
                spans.push((s as f64 * frame_shift, i as f64 * frame_shift));
            }
            run_start = None;
        }
    }
    if let Some(s) = run_start {
        if (frames.len() - s) as f64 * frame_shift >= min_span_secs {
            spans.push((s as f64 * frame_shift, frames.len() as f64 * frame_shift));
        }
    }
    spans
}

fn normalize_to(x: &mut [f32], target: f32) {
    let r = rms(x);
    if r > 1e-6 {
        let g = target / r;
        for v in x.iter_mut() {
            *v *= g;
        }
    }
}

/// Raw (un-normalized) span slices for the two streams, replicating the
/// production separator's geometry exactly (span ± SEPARATION_CONTEXT_SECS
/// context, span carved from each stream's timeline) but skipping the
/// RMS normalization — the census needs the PRE-normalization ratio.
fn separate_raw(
    session: &mut ort::session::Session,
    samples: &[f32],
    span: (f64, f64),
) -> anyhow::Result<(Vec<Vec<f32>>, f32)> {
    let sr = SR;
    let c0 = (((span.0 - SEPARATION_CONTEXT_SECS).max(0.0)) * sr) as usize;
    let c1 = (((span.1 + SEPARATION_CONTEXT_SECS) * sr) as usize).min(samples.len());
    let clip = &samples[c0..c1];
    let clip_rms = rms(clip);
    let input_2d: Array2<f32> = ndarray::Array1::from(clip.to_vec())
        .into_shape_with_order([1, clip.len()])
        .map_err(|e| anyhow::anyhow!("input shape: {e}"))?;
    let t = TensorRef::from_array_view(input_2d.view()).map_err(|e| anyhow::anyhow!("tensor: {e}"))?;
    let outputs = session.run(ort::inputs!["mix" => t]).map_err(|e| anyhow::anyhow!("forward: {e}"))?;
    let out = outputs
        .get("est_source")
        .ok_or_else(|| anyhow::anyhow!("output est_source missing"))?;
    let arr = out
        .try_extract_array::<f32>()
        .map_err(|e| anyhow::anyhow!("extract: {e}"))?;
    let shape = arr.shape();
    if shape[1] != 2 {
        return Err(anyhow::anyhow!("expected 2 sources, got {}", shape[1]));
    }
    let slice: &[f32] = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());
    let s0 = ((span.0 * sr) as usize).max(c0);
    let s1 = ((span.1 * sr) as usize).min(c1);
    let carve0 = s0 - c0;
    let carve1 = carve0 + (s1 - s0);
    let per_src = shape[2];
    if carve1 > per_src.min(c1 - c0) {
        return Err(anyhow::anyhow!("output too short to carve the span"));
    }
    let mut streams = Vec::with_capacity(2);
    for s in 0..2 {
        streams.push(slice[s * per_src + carve0..s * per_src + carve1].to_vec());
    }
    Ok((streams, clip_rms))
}

#[test]
#[ignore = "live diag: MEETIFY_LIVE_DIAG=1 cargo test -j 2 --test overlap_census_probe -- --ignored --nocapture"]
fn overlap_census_dry_run() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = resolve_audio();
    let samples = load_samples(&audio_path);

    let models_dir = std::path::Path::new(&home).join(".meetily-models");
    let pya = PyannoteSegmentation::new(
        models_dir.join("pyannote-segmentation.onnx").to_str().unwrap(),
    )
    .expect("pyannote segmentation");

    // Frame masses: same cache file the ear gate uses (same provenance), so
    // the ~16-min pyannote pass is paid once across all diagnostics.
    let cache_path = audio_path.parent().unwrap().join("gate_frame_masses.json");
    let prov = cache_provenance(
        models_dir.join("pyannote-segmentation.onnx").as_path(),
    )
    .expect("cache provenance");
    let t0 = std::time::Instant::now();
    let fm = match FrameMassesOutput::load(&cache_path, &prov) {
        Ok(fm) => {
            eprintln!("CENSUS: frame masses from cache");
            fm
        }
        Err(_) => {
            let fm = pya.frame_masses(&samples).expect("frame masses");
            eprintln!(
                "CENSUS: pyannote pass in {:.0}s — cached to {}",
                t0.elapsed().as_secs_f64(),
                cache_path.display()
            );
            fm.save(&cache_path, &prov).expect("save frame cache");
            fm
        }
    };

    // ── S16 per-frame overlap-mass profile (token-only numbers).
    eprintln!(
        "CENSUS S16-PROFILE [{:.1}-{:.1}] (trigger bar {OVERLAP_TRIGGER_MASS}):",
        S16_PROFILE.0, S16_PROFILE.1
    );
    let p0 = (S16_PROFILE.0 / FRAME_SHIFT) as usize;
    let p1 = ((S16_PROFILE.1 / FRAME_SHIFT) as usize).min(fm.frames.len());
    let mut s16_max_mass = 0.0f32;
    for (i, f) in fm.frames.iter().enumerate().skip(p0).take(p1 - p0) {
        eprintln!("CENSUS S16 t={:.3} overlap={:.3}", i as f64 * FRAME_SHIFT, f.overlap);
        s16_max_mass = s16_max_mass.max(f.overlap);
    }

    // ── The trigger, verbatim production semantics.
    let spans = overlap_spans(&fm.frames, FRAME_SHIFT);
    eprintln!("CENSUS: {} trigger-fired span(s) at mass ≥ {OVERLAP_TRIGGER_MASS}, ≥0.4s", spans.len());
    for (s0, s1) in &spans {
        let f0 = (s0 / FRAME_SHIFT) as usize;
        let f1 = ((s1 / FRAME_SHIFT) as usize).min(fm.frames.len());
        let mut mx = 0.0f32;
        let mut sum = 0.0f32;
        for f in &fm.frames[f0..f1] {
            mx = mx.max(f.overlap);
            sum += f.overlap;
        }
        let mean = sum / (f1 - f0).max(1) as f32;
        eprintln!(
            "CENSUS SPAN [{:.2}-{:.2}] mass_max={:.3} mass_mean={:.3}",
            s0, s1, mx, mean
        );
    }
    let s16_fires = spans
        .iter()
        .any(|&(s0, s1)| s0 < SPAN_S16.1 && SPAN_S16.0 < s1);
    eprintln!(
        "CENSUS S16-FIRES: {} (window max mass {:.3} vs bar {OVERLAP_TRIGGER_MASS})",
        if s16_fires { "YES" } else { "NO" },
        s16_max_mass
    );

    // ── Dial sweep: span count (and S16 firing) at candidate trigger
    //    settings, production semantics parameterized. Separation cost
    //    scales with span count — this predicts the blow-up before any
    //    dial moves.
    eprintln!("CENSUS SWEEP (mass_bar duration → spans, s16_fires):");
    for &(mass, dur) in &[
        (0.5f32, 0.4f64),
        (0.5, 0.3),
        (0.5, 0.25),
        (0.5, 0.2),
        (0.5, 0.1),
        (0.4, 0.4),
        (0.4, 0.3),
        (0.3, 0.4),
    ] {
        let sw = overlap_spans_at(&fm.frames, FRAME_SHIFT, mass, dur);
        let fires = sw.iter().any(|&(a, b)| a < SPAN_S16.1 && SPAN_S16.0 < b);
        eprintln!(
            "CENSUS SWEEP mass={:.2} dur={:.2} → spans={} s16={}",
            mass,
            dur,
            sw.len(),
            if fires { "FIRES" } else { "no" }
        );
    }

    // ── Per-span stream identity (raw session, production geometry).
    let sep_model = app_lib::audio::speaker::model_download::separation_model_path();
    assert!(sep_model.exists(), "separation model missing: {}", sep_model.display());
    let providers = vec![CPUExecutionProvider::default().build()];
    let mut session = ort::session::Session::builder()
        .expect("builder")
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .expect("opt")
        .with_execution_providers(providers)
        .expect("providers")
        .with_intra_threads(4)
        .expect("threads")
        .commit_from_file(&sep_model)
        .expect("commit separation session");

    let extractor = NemoEmbeddingExtractor::new(
        models_dir
            .join(app_lib::audio::speaker::model_download::embedding_filename())
            .to_str()
            .unwrap(),
    )
    .expect("titanet extractor");
    let refs: [(&'static str, Vec<f32>); 3] = [
        ("userA", {
            extractor
                .extract_embedding(
                    &samples[(REF_USERA.0 * SR) as usize..(REF_USERA.1 * SR) as usize],
                    16_000,
                )
                .expect("ref userA")
        }),
        ("userB", {
            extractor
                .extract_embedding(
                    &samples[(REF_USERB.0 * SR) as usize..(REF_USERB.1 * SR) as usize],
                    16_000,
                )
                .expect("ref userB")
        }),
        ("userC", {
            extractor
                .extract_embedding(
                    &samples[(REF_USERC.0 * SR) as usize..(REF_USERC.1 * SR) as usize],
                    16_000,
                )
                .expect("ref userC")
        }),
    ];

    let mut both_decisive_distinct = 0usize;
    let mut reject_kinds = (0usize, 0usize, 0usize); // (margin-miss, collapsed-stream, same-badge)
    for &(s0, s1) in &spans {
        let (raw_streams, clip_rms) =
            separate_raw(&mut session, &samples, (s0, s1)).expect("separate span");
        // The separator's raw output scale is a per-forward artifact (export
        // gain), so absolute raw RMS is meaningless — the WITHIN-SPAN energy
        // split between the two streams is the silence signal: a collapsed
        // (residue-only) stream sits near 0 or 1.
        let raw_rms: Vec<f32> = raw_streams.iter().map(|r| rms(r)).collect();
        let total: f32 = raw_rms.iter().sum();
        let mut tops: Vec<&'static str> = Vec::new();
        let mut decisive = true;
        for (i, raw) in raw_streams.iter().enumerate() {
            let split = raw_rms[i] / total.max(1e-9);
            let mut norm = raw.clone();
            normalize_to(&mut norm, clip_rms);
            let emb = extractor
                .extract_embedding(&norm, 16_000)
                .expect("stream embedding");
            let mut coss: Vec<(&'static str, f32)> =
                refs.iter().map(|(n, r)| (*n, cosine(r, &emb))).collect();
            coss.sort_by(|a, b| b.1.total_cmp(&a.1));
            let margin = coss[0].1 - coss[1].1;
            let ok = margin >= SUBTURN_VOTE_MARGIN;
            decisive = decisive && ok;
            if !ok {
                reject_kinds.0 += 1;
            }
            if split < 0.05 || split > 0.95 {
                reject_kinds.1 += 1;
            }
            eprintln!(
                "CENSUS READ span[{:.2}-{:.2}] stream{i} energy_split={:.3} top={} c1={:.4} c2={:.4} margin={:.4} {}",
                s0,
                s1,
                split,
                coss[0].0,
                coss[0].1,
                coss[1].1,
                margin,
                if ok { "DECISIVE" } else { "REJECT" }
            );
            tops.push(coss[0].0);
        }
        let distinct = tops[0] != tops[1];
        if !distinct {
            reject_kinds.2 += 1;
        }
        if decisive && distinct {
            both_decisive_distinct += 1;
        }
        eprintln!(
            "CENSUS VERDICT span[{:.2}-{:.2}] both_decisive={} distinct={} → {}",
            s0,
            s1,
            decisive,
            distinct,
            if decisive && distinct { "SYNTHESIZE" } else { "DEGRADE" }
        );
    }
    eprintln!(
        "CENSUS SUMMARY: fired={} both_decisive_distinct={} rejects(margin,silent,same-badge)=({},{},{}) s16_fires={}",
        spans.len(),
        both_decisive_distinct,
        reject_kinds.0,
        reject_kinds.1,
        reject_kinds.2,
        s16_fires
    );
}
