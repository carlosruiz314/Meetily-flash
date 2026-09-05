//! Cross-meeting reference miner (change `hybrid-diarization-engine`):
//! enrolls voice references from ANOTHER meeting on disk by clustering its
//! voices and matching them against the reference meeting's cluster
//! centroids via TitaNet affinity.
//!
//! MEETIFY_MINE_AUDIO=<path> MEETIFY_MINE_DUMP=<out.json> \
//!   MEETIFY_LIVE_DIAG=1 cargo test --test reference_miner -- --ignored --nocapture
//!
//! Matched centroids are dumped as `{"Carlos Ruiz": [...], "Cynthia Wu": [...]}`
//! for enrollment seeding. Identities are algorithmic (affinity margin) and
//! confirmed by the user at review.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::{local_labels, PyannoteSegmentation};
use app_lib::audio::speaker::run_assembly::{
    cluster_pieces, derive_pieces, drop_textless, embed_slice, merge_to_cap, refine_loop,
    speech_runs, MODE_FILTER_RADIUS_FRAMES, SPEECH_GATE,
};
use std::collections::BTreeMap;

const MATCH_MARGIN: f32 = 0.08;

fn cos(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= 0.0 || nb <= 0.0 { 0.0 } else { dot / (na * nb) }
}

#[tokio::test]
#[ignore = "live miner: MEETIFY_MINE_AUDIO=... cargo test --test reference_miner -- --ignored --nocapture"]
async fn mine_references() {
    eprintln!(
        "MINE: start — MEETIFY_LIVE_DIAG={:?} MEETIFY_MINE_AUDIO={:?}",
        std::env::var("MEETIFY_LIVE_DIAG"),
        std::env::var("MEETIFY_MINE_AUDIO")
    );
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = std::env::var("MEETIFY_MINE_AUDIO").expect("MEETIFY_MINE_AUDIO");
    let match_path = std::env::var("MEETIFY_MINE_MATCH").expect("MEETIFY_MINE_MATCH");
    let dump_path = std::env::var("MEETIFY_MINE_DUMP").expect("MEETIFY_MINE_DUMP");

    // Target identities: the reference meeting's cluster centroids.
    // (Names mirror the cde5c264 analysis: 0=Carlos, 1=Cynthia, 2=Ricardo.)
    let target: BTreeMap<String, Vec<f32>> =
        serde_json::from_str(&std::fs::read_to_string(&match_path).expect("read match file"))
            .expect("parse match file");
    let targets: Vec<(&str, &Vec<f32>)> =
        target.iter().map(|(k, v)| (k.as_str(), v)).collect();

    let home = std::env::var("USERPROFILE").unwrap();
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{}/{}/{}",
        home,
        ".meetily-models",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");
    let pya = PyannoteSegmentation::new(&format!(
        "{home}/.meetily-models/pyannote-segmentation.onnx"
    ))
    .expect("segmentation model");

    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode candidate audio");
    let samples = decoded.to_whisper_format();
    eprintln!("MINE: candidate {:.1}s", decoded.duration_seconds);

    // Voice-activity spans keep every speech piece (mining wants coverage,
    // not text alignment).
    let vad = app_lib::audio::vad::get_speech_chunks(&samples, 2000).expect("vad");
    let spans: Vec<(f64, f64)> = vad
        .iter()
        .map(|s| (s.start_timestamp_ms as f64 / 1000.0, s.end_timestamp_ms as f64 / 1000.0))
        .collect();

    let fm = pya.frame_masses(&samples).expect("frame masses");
    let labels = local_labels(&fm.frames, SPEECH_GATE);
    let runs = speech_runs(&fm.frames, app_lib::audio::speaker::pyannote_segmentation::FRAME_SHIFT);
    let pieces = derive_pieces(
        &labels,
        &runs,
        &fm.window_label_tracks,
        app_lib::audio::speaker::pyannote_segmentation::FRAME_SHIFT,
        MODE_FILTER_RADIUS_FRAMES,
    );
    let kept = drop_textless(&pieces, &spans, 0.25);
    eprintln!("MINE: {} pieces kept", kept.len());

    let sr = 16_000u32;
    let srf = sr as f64;
    let mut embs: Vec<Vec<f32>> = Vec::new();
    for p in &kept {
        let dur = p.end_secs - p.start_secs;
        let (oa, ob) = embed_slice(dur);
        let i0 = ((p.start_secs + oa) * srf) as usize;
        let i1 = (((p.start_secs + ob) * srf) as usize).min(samples.len());
        if i1 > i0 {
            if let Some(e) = extractor.extract_embedding(&samples[i0..i1], sr) {
                embs.push(e);
            }
        }
    }
    eprintln!("MINE: {} piece embeddings", embs.len());

    // Cluster the candidate's own voices, then refine against the target
    // centroids so pieces gravitate to the person they match cross-meeting.
    let (mut assign, mut centroids) = cluster_pieces(&embs, 0.65);
    let target_vecs: Vec<Vec<f32>> = targets.iter().map(|(_, v)| (*v).clone()).collect();
    for tv in &target_vecs {
        centroids.push(tv.clone());
    }
    refine_loop(&mut assign, &mut centroids, &embs, 10);

    // Report affinity of every final centroid to every target identity.
    let mut matched: BTreeMap<String, Vec<f32>> = BTreeMap::new();
    for (ci, c) in centroids.iter().enumerate() {
        let member_count = assign.iter().filter(|a| **a == ci).count();
        if member_count == 0 {
            continue;
        }
        let mut sims: Vec<(&str, f32)> =
            targets.iter().map(|(n, v)| (*n, cos(c, v))).collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let margin = if sims.len() >= 2 { sims[0].1 - sims[1].1 } else { f32::INFINITY };
        eprintln!(
            "MINE: centroid {ci} ({} pieces) best={} {:.3} second={:.3} margin={:.3}",
            member_count, sims[0].0, sims[0].1, sims.get(1).map(|s| s.1).unwrap_or(0.0), margin
        );
        // A target cluster (id < target_vecs.len()) may match its own identity.
        if sims[0].1 >= 0.30 && margin >= MATCH_MARGIN && member_count >= 2 {
            let name = sims[0].0;
            if let Some(existing) = matched.get(name) {
                let better =
                    cos(c, target[name].as_slice()) > cos(existing, target[name].as_slice());
                if !better {
                    continue;
                }
            }
            matched.insert(name.to_string(), c.clone());
        }
    }
    eprintln!("MINE: matched identities: {:?}", matched.keys().collect::<Vec<_>>());
    std::fs::write(&dump_path, serde_json::to_string_pretty(&matched).expect("serialize"))
        .expect("write dump");
    eprintln!("MINE: wrote {}", dump_path);
}
