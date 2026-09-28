//! overlap-separation-prepass task 2.2: in-domain quality stop-gate.
//!
//! Runs the exported Conv-TasNet checkpoint (task 2.1) over the attested
//! overlap windows of the cde5c264 recording and scores the separated
//! streams against solo-window TitaNet references. The stop-gate question:
//! does separation BEAT the mixture for the overlapping voice, and does the
//! S7c atom rescore decisively? If no criterion clears, Phase 2 stops here
//! and S16 stays the accepted limitation.
//!
//! All audio/model access is local (recording folder + models dir, resolved
//! at runtime — no meeting-derived data enters the repo).

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::run_assembly::cosine;
use ndarray::Array2;
use ort::execution_providers::CPUExecutionProvider;
use ort::session::builder::GraphOptimizationLevel;
use ort::value::TensorRef;

const MODELS_DIR: &str = ".meetily-models";
const SEPARATION_MODEL: &str = "conv_tasnet_libri2mix_sepnoisy_16k.onnx";
const SR: f64 = 16_000.0;

// Attested windows (seconds) — numbers already carried by the scrubbed test
// code and the fixture's public shape.
/// The measured real overlap (0.83 powerset mass).
const SPAN_MEASURED: (f64, f64) = (34.5, 34.9);
/// Neighborhood run so the separated streams cover the S7c atom.
const SPAN_S7C_NEIGHBORHOOD: (f64, f64) = (34.0, 37.0);
/// The attested crosstalk exchange (S16).
const SPAN_S16: (f64, f64) = (1055.5, 1057.5);

// Solo reference windows (fixture-pinned single-voice rows).
const REF_USERA: (f64, f64) = (46.0, 50.0);
const REF_USERB: (f64, f64) = (1196.2, 1199.5);
const REF_USERC: (f64, f64) = (1201.0, 1210.0);
/// The S7c atom ("I don't know", userA).
const S7C_ATOM: (f64, f64) = (36.0, 36.8);

/// Recording folder: `Music/*-recordings/<meeting>/audio.mp4`. The root's
/// name varies and is deliberately not hardcoded here; the env var wins.
fn resolve_audio() -> std::path::PathBuf {
    let meeting = "Meeting 2026-06-22_16-04-01_2026-06-22_14-04";
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

/// 16 kHz mono samples — reuses the ear gate's provenance-pinned samples
/// cache beside the audio (decode costs minutes; the cache meta pins the
/// source audio's sha256).
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
        match meta_json.get("audio_sha256").and_then(|v| v.as_str()) {
            Some(sha) if sha == source_sha => {
                let bytes = std::fs::read(&cache).expect("read samples cache");
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                eprintln!(
                    "PROBE: samples from cache ({} = {:.1}s, provenance OK)",
                    samples.len(),
                    samples.len() as f64 / SR
                );
                return samples;
            }
            _ => eprintln!("PROBE: samples cache provenance mismatch — re-decoding"),
        }
    }
    let decoded = app_lib::audio::decoder::decode_audio_file(audio_path).expect("decode audio");
    let s = decoded.to_whisper_format();
    let bytes: Vec<u8> = s.iter().flat_map(|v| v.to_le_bytes()).collect();
    std::fs::write(&cache, &bytes).expect("write samples cache");
    let meta_json =
        serde_json::json!({ "samples": s.len(), "audio_sha256": source_sha });
    std::fs::write(&meta, meta_json.to_string()).expect("write samples meta");
    eprintln!("PROBE: decoded and cached {:.1}s", decoded.duration_seconds);
    s
}

/// Two separated streams for the span (Conv-TasNet output preserves the
/// input length; trim defensively anyway).
fn separate(
    session: &mut ort::session::Session,
    samples: &[f32],
    span: (f64, f64),
) -> anyhow::Result<Vec<Vec<f32>>> {
    let i0 = (span.0 * SR) as usize;
    let i1 = ((span.1 * SR) as usize).min(samples.len());
    let clip = &samples[i0..i1];
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
    assert_eq!(shape[1], 2, "expected a 2-source model");
    let slice: &[f32] = arr.as_slice().unwrap_or_else(|| arr.to_slice().unwrap());
    let per_src = shape[2].min(clip.len());
    let mut streams = Vec::with_capacity(2);
    for s in 0..2 {
        streams.push(slice[s * shape[2]..s * shape[2] + per_src].to_vec());
    }
    Ok(streams)
}

fn cosines_to_refs(emb: &[f32], refs: &[(&'static str, Vec<f32>); 3]) -> Vec<(&'static str, f32)> {
    refs.iter().map(|(n, r)| (*n, cosine(r, emb))).collect()
}

fn embed_window(
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    window: (f64, f64),
) -> Vec<f32> {
    let i0 = (window.0 * SR) as usize;
    let i1 = ((window.1 * SR) as usize).min(samples.len());
    extractor
        .extract_embedding(&samples[i0..i1], 16_000)
        .expect("embedding")
}

fn ranked(coss: &[(&'static str, f32)]) -> Vec<(&'static str, f32)> {
    let mut v = coss.to_vec();
    v.sort_by(|a, b| b.1.total_cmp(&a.1));
    v
}

fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len().max(1) as f32).sqrt()
}

/// Scale a separated stream to the mixture window's RMS — TitaNet is
/// log-compressed, so level shifts shift cosines; the model's output gain
/// is an artifact, not voice information.
fn normalize_to(x: &mut [f32], target: f32) {
    let r = rms(x);
    if r > 1e-6 {
        let g = target / r;
        for v in x.iter_mut() {
            *v *= g;
        }
    }
}

#[test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --test overlap_separation_quality_probe -- --ignored --nocapture"]
fn overlap_separation_quality_stopgate() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = resolve_audio();
    let samples = load_samples(&audio_path);

    let sep_model = std::path::Path::new(&home)
        .join(MODELS_DIR)
        .join(SEPARATION_MODEL);
    assert!(sep_model.exists(), "separation model missing: {}", sep_model.display());
    let providers = vec![CPUExecutionProvider::default().build()];
    let mut session = ort::session::Session::builder()
        .expect("builder")
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .expect("opt")
        .with_execution_providers(providers)
        .expect("providers")
        .with_intra_threads(2)
        .expect("threads")
        .commit_from_file(&sep_model)
        .expect("commit separation session");

    let extractor = NemoEmbeddingExtractor::new(
        std::path::Path::new(&home)
            .join(MODELS_DIR)
            .join(app_lib::audio::speaker::model_download::embedding_filename())
            .to_str()
            .unwrap(),
    )
    .expect("titanet extractor");

    let refs: [(&'static str, Vec<f32>); 3] = [
        ("userA", {
            let i0 = (REF_USERA.0 * SR) as usize;
            let i1 = (REF_USERA.1 * SR) as usize;
            extractor
                .extract_embedding(&samples[i0..i1], 16_000)
                .expect("ref userA")
        }),
        ("userB", {
            let i0 = (REF_USERB.0 * SR) as usize;
            let i1 = (REF_USERB.1 * SR) as usize;
            extractor
                .extract_embedding(&samples[i0..i1], 16_000)
                .expect("ref userB")
        }),
        ("userC", {
            let i0 = (REF_USERC.0 * SR) as usize;
            let i1 = (REF_USERC.1 * SR) as usize;
            extractor
                .extract_embedding(&samples[i0..i1], 16_000)
                .expect("ref userC")
        }),
    ];

    // ── Criterion 1: measured span — a stream must beat the mixture on userA.
    // Evaluated twice: the production 0.4s window, and the [34.5,34.9]
    // sub-window carved from the 3s neighborhood separation (context helps
    // short-span separation; this diagnoses whether c1 fails on window
    // length or on the model itself).
    let mut c1_results: Vec<(String, f32, f32)> = Vec::new();
    {
        let mix_rms = rms(&samples[(SPAN_MEASURED.0 * SR) as usize..(SPAN_MEASURED.1 * SR) as usize]);
        let mix_c = cosines_to_refs(&embed_window(&extractor, &samples, SPAN_MEASURED), &refs);
        eprintln!("PROBE [34.5-34.9] mixture cosines: {}", fmt(&mix_c));
        let mix_a = ranked(&mix_c).iter().find(|(n, _)| *n == "userA").unwrap().1;

        let streams = separate(&mut session, &samples, SPAN_MEASURED).expect("separate");
        for (i, s0) in streams.iter().enumerate() {
            eprintln!("PROBE [34.5-34.9] stream{i} rms {} vs mix {mix_rms:.5}", rms(s0));
            let mut norm = s0.clone();
            normalize_to(&mut norm, mix_rms);
            let emb = extractor.extract_embedding(&norm, 16_000).expect("stream embedding");
            eprintln!("PROBE [34.5-34.9] stream{i} (rms-norm) cosines: {}", fmt(&cosines_to_refs(&emb, &refs)));
        }
        let best_stream_a = streams
            .iter()
            .map(|s0| {
                let mut norm = s0.clone();
                normalize_to(&mut norm, mix_rms);
                let emb = extractor.extract_embedding(&norm, 16_000).unwrap();
                refs.iter().find(|(n, _)| *n == "userA").map(|(_, r)| cosine(r, &emb)).unwrap()
            })
            .fold(f32::MIN, f32::max);
        c1_results.push(("production-span".into(), best_stream_a, mix_a));

        let nb_streams = separate(&mut session, &samples, SPAN_S7C_NEIGHBORHOOD).expect("separate nb");
        let nb_mix_rms = rms(&samples[(SPAN_MEASURED.0 * SR) as usize..(SPAN_MEASURED.1 * SR) as usize]);
        let offset = ((SPAN_MEASURED.0 - SPAN_S7C_NEIGHBORHOOD.0) * SR) as usize;
        let len = ((SPAN_MEASURED.1 - SPAN_MEASURED.0) * SR) as usize;
        let best_nb = nb_streams
            .iter()
            .map(|s0| {
                let mut sub = s0[offset..offset + len].to_vec();
                normalize_to(&mut sub, nb_mix_rms);
                let emb = extractor.extract_embedding(&sub, 16_000).unwrap();
                refs.iter().find(|(n, _)| *n == "userA").map(|(_, r)| cosine(r, &emb)).unwrap()
            })
            .fold(f32::MIN, f32::max);
        c1_results.push(("neighborhood-carved".into(), best_nb, mix_a));

        for (label, best, mix) in &c1_results {
            eprintln!("STOP-GATE c1 [{label}]: best stream userA cosine {best:.4} vs mixture {mix:.4}");
        }
        assert!(
            c1_results.iter().any(|(_, best, mix)| best > mix),
            "STOP-GATE c1 FAILED: no separated stream beats the mixture on the userA reference in any window configuration ({c1_results:?})"
        );
    }

    // ── Criterion 2: S7c atom rescores decisively for userA in the
    //    userA-identified stream. Stream identity comes from the span-level
    //    embedding; the atom is then carved from that stream's timeline.
    {
        let streams = separate(&mut session, &samples, SPAN_S7C_NEIGHBORHOOD).expect("separate");
        let nb_rms = rms(&samples[(SPAN_S7C_NEIGHBORHOOD.0 * SR) as usize..(SPAN_S7C_NEIGHBORHOOD.1 * SR) as usize]);
        let mut identities = Vec::new();
        for (i, s0) in streams.iter().enumerate() {
            let mut norm = s0.clone();
            normalize_to(&mut norm, nb_rms);
            let emb = extractor.extract_embedding(&norm, 16_000).unwrap();
            let c = ranked(&cosines_to_refs(&emb, &refs));
            eprintln!("PROBE [34.0-37.0] stream{i} span-level: {}", fmt(&c));
            identities.push(c[0].0);
        }
        // carve the S7c atom from each stream and rescore
        let a0 = ((S7C_ATOM.0 - SPAN_S7C_NEIGHBORHOOD.0) * SR) as usize;
        let a1 = ((S7C_ATOM.1 - SPAN_S7C_NEIGHBORHOOD.0) * SR) as usize;
        let atom_rms = rms(&samples[(S7C_ATOM.0 * SR) as usize..(S7C_ATOM.1 * SR) as usize]);
        let mut atom_reads = Vec::new();
        for (i, s0) in streams.iter().enumerate() {
            let mut sub = s0[a0..a1].to_vec();
            normalize_to(&mut sub, atom_rms);
            let emb = extractor.extract_embedding(&sub, 16_000).unwrap();
            let c = ranked(&cosines_to_refs(&emb, &refs));
            eprintln!("PROBE S7c atom in stream{i} ({}): {}", identities[i], fmt(&c));
            atom_reads.push((identities[i].clone(), c[0].0, c[0].1 - c[1].1));
        }
        let usera_stream = atom_reads
            .iter()
            .find(|(id, _, _)| *id == "userA")
            .expect("a userA-identified stream");
        eprintln!(
            "STOP-GATE c2: S7c atom in the userA stream → {} margin {:.4}",
            usera_stream.1, usera_stream.2
        );
        assert!(
            usera_stream.1 == "userA" && usera_stream.2 >= 0.05,
            "STOP-GATE c2 FAILED: S7c atom does not rescore decisively for userA in the userA stream ({atom_reads:?})"
        );
    }

    // ── Criterion 3 (diagnostic): S16 crosstalk separates into two distinct
    //    voices — the two streams' best references must differ.
    {
        let streams = separate(&mut session, &samples, SPAN_S16).expect("separate");
        let mut tops = Vec::new();
        let s16_rms = rms(&samples[(SPAN_S16.0 * SR) as usize..(SPAN_S16.1 * SR) as usize]);
        for (i, s) in streams.iter().enumerate() {
            let mut norm = s.clone();
            normalize_to(&mut norm, s16_rms);
            let emb = extractor
                .extract_embedding(&norm, 16_000)
                .expect("stream embedding");
            let mut c = cosines_to_refs(&emb, &refs);
            c.sort_by(|a, b| b.1.total_cmp(&a.1));
            eprintln!(
                "PROBE [1055.5-1057.5] stream{i}: {} (margin {:.4})",
                fmt(&c),
                c[0].1 - c[1].1
            );
            tops.push(c[0].0);
        }
        tops.sort();
        tops.dedup();
        eprintln!("STOP-GATE c3: distinct stream identities: {tops:?}");
        assert_eq!(
            tops.len(),
            2,
            "STOP-GATE c3 FAILED: S16 streams did not resolve to two distinct voices ({tops:?})"
        );
    }

    eprintln!("STOP-GATE: all criteria passed — separation quality clears the bar");
}

fn fmt(coss: &[(&'static str, f32)]) -> String {
    coss.iter()
        .map(|(n, c)| format!("{n}={c:.4}"))
        .collect::<Vec<_>>()
        .join(", ")
}
