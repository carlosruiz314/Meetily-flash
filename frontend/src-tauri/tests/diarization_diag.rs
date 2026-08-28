//! Ear-truth diagnostic for speaker attribution quality (env-gated).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --test diarization_diag -- --ignored --nocapture
//!
//! The user ear-verified that the words "five years" at ~12.1s belong to the
//! SAME voice as the 5.9-12.0s turn (one continuous sentence), while the
//! pipeline labels them to a different speaker. This harness measures, on the
//! real audio:
//!   1. all-pairs cosine between probe windows (incl. that slice),
//!   2. each probe's similarity to the three final cluster centroids,
//!   3. per-final-segment margin (similarity to own centroid vs best other)
//!      so "how much of this meeting is ambiguous" becomes a number.

use app_lib::audio::speaker::diarization::DiarizationPort;
use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::sherpa_adapter::OrtDiarizationAdapter;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;

const MODELS_DIR: &str = ".meetily-models";
const AUDIO: &str = "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const THRESHOLD: f32 = 0.65;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= 0.0 || nb <= 0.0 { 0.0 } else { dot / (na * nb) }
}

#[tokio::test]
#[ignore = "live GPU pass: MEETIFY_LIVE_DIAG=1 cargo test --test diarization_diag -- --ignored --nocapture"]
async fn diarization_attribution_diag_cde5c264() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let models_dir = format!("{}/{}", home(), MODELS_DIR);
    let audio_path = format!("{}/{}", home(), AUDIO);

    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("DIAG: decoded {:.1}s", decoded.duration_seconds);

    let speech = app_lib::audio::vad::get_speech_chunks(&samples, 2000).expect("vad");
    let regions: Vec<(f64, f64)> = speech
        .iter()
        .map(|s| (s.start_timestamp_ms / 1000.0, s.end_timestamp_ms / 1000.0))
        .collect();
    eprintln!("DIAG: {} speech regions", regions.len());

    let emb_path = format!("{}/{}", models_dir, app_lib::audio::speaker::model_download::embedding_filename());
    let seg_path = format!("{}/pyannote-segmentation.onnx", models_dir);
    let fp = (THRESHOLD * 65536.0) as u32;
    let adapter = OrtDiarizationAdapter::with_shared_threshold(&emb_path, &seg_path, Arc::new(AtomicU32::new(fp)))
        .expect("adapter");

    let out = adapter
        .process(&samples, 16_000, &regions)
        .expect("diarization process");
    let mut centroids = out.centroids.clone();
    eprintln!("DIAG: coarse segments={} centroids={}", out.segments.len(), centroids.len());

    let final_segments = adapter.refine_pass2(&samples, 16_000, &centroids).expect("pass2");
    // keep only centroids that survived into final segments
    let used: std::collections::HashSet<u32> = final_segments.iter().map(|s| s.speaker_id).collect();
    centroids.retain(|k, _| used.contains(k));
    eprintln!("DIAG: final segments={} centroids={}", final_segments.len(), centroids.len());

    // Extractor for probes and per-segment margins.
    let extractor = NemoEmbeddingExtractor::new(&emb_path).expect("extractor");
    let slice = |a: f64, b: f64| -> Vec<f32> {
        let i0 = (a * 16_000.0) as usize;
        let i1 = ((b * 16_000.0) as usize).min(samples.len());
        samples[i0..i1].to_vec()
    };

    // Probes: A/C = Sp0 turns flanking the ear-verified misattributed slice.
    let probes: Vec<(&str, f64, f64)> = vec![
        ("A_sp0_5.9-12.0", 5.9, 12.0),
        ("B_five_years_12.07-12.8", 12.1, 12.8),
        ("C_rest_of_row_12.8-15.8", 12.85, 15.8),
        ("D_sp0_16.2-20.1", 16.25, 20.1),
        ("E_sp2_2803-2818", 2803.5, 2818.0),
        ("F_sp0_2820-2848", 2821.0, 2848.0),
    ];
    let mut probe_emb: Vec<(&str, Vec<f32>)> = Vec::new();
    for (name, a, b) in &probes {
        if let Some(e) = extractor.extract_embedding(&slice(*a, *b), 16_000) {
            probe_emb.push((name, e));
        } else {
            eprintln!("DIAG: probe {name} returned None (silence/too short)");
        }
    }

    eprintln!("DIAG: --- probe all-pairs cosine ---");
    for i in 0..probe_emb.len() {
        for j in (i + 1)..probe_emb.len() {
            eprintln!(
                "  {} vs {}: {:.3}",
                probe_emb[i].0, probe_emb[j].0,
                cosine(&probe_emb[i].1, &probe_emb[j].1)
            );
        }
    }

    eprintln!("DIAG: --- probe vs final centroids ---");
    let mut cids: Vec<&u32> = centroids.keys().collect();
    cids.sort();
    for (name, e) in &probe_emb {
        let mut sims: Vec<(u32, f32)> = cids
            .iter()
            .map(|c| (**c, cosine(e, &centroids[*c])))
            .collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let row: Vec<String> = sims.iter().map(|(c, s)| format!("sp{c}={s:.3}")).collect();
        eprintln!("  {}: {}", name, row.join(" "));
    }

    // Per-final-segment margin: own-centroid vs best-other on a re-extracted
    // embedding of the segment's audio (sampled windows for long segments).
    eprintln!("DIAG: --- per-segment margins ---");
    let mut margins: Vec<(f64, f64, u32, f32)> = Vec::new();
    for seg in &final_segments {
        let dur = seg.end_seconds - seg.start_seconds;
        let (a, b) = if dur > 12.0 {
            let mid = (seg.start_seconds + seg.end_seconds) / 2.0;
            (mid - 6.0, mid + 6.0)
        } else {
            (seg.start_seconds, seg.end_seconds)
        };
        let Some(e) = extractor.extract_embedding(&slice(a, b), 16_000) else {
            continue;
        };
        let mut sims: Vec<(u32, f32)> = cids
            .iter()
            .map(|c| (**c, cosine(&e, &centroids[*c])))
            .collect();
        sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let own = sims.iter().find(|(c, _)| *c == seg.speaker_id).map(|x| x.1).unwrap_or(0.0);
        let best_other = sims.iter().find(|(c, _)| *c != seg.speaker_id).map(|x| x.1).unwrap_or(0.0);
        margins.push((seg.start_seconds, seg.end_seconds, seg.speaker_id, own - best_other));
    }
    let total = margins.len();
    let ambiguous = margins.iter().filter(|(_, _, _, m)| *m < 0.05).count();
    let wrong_own = margins.iter().filter(|(_, _, _, m)| *m < 0.0).count();
    eprintln!(
        "DIAG: segments-with-embedding={total} margin<0.05={ambiguous} ({:.1}%) margin<0 (own NOT nearest)={wrong_own}",
        100.0 * ambiguous as f64 / total.max(1) as f64
    );


    // Calibration: are 0.7s slices of KNOWN same-speaker turns self-consistent?
    // If Speaker 0's own short slices don't match Speaker 0's centroid, then
    // sub-second attribution is noise and every banter tail-word label is
    // meaningless.
    eprintln!("DIAG: --- short-slice calibration (0.7s windows) ---");
    let cal_windows: Vec<(&str, f64, f64)> = vec![
        ("D_w1", 16.25, 16.95), ("D_w2", 16.95, 17.65), ("D_w3", 17.65, 18.35),
        ("D_w4", 18.35, 19.05), ("D_w5", 19.05, 19.75),
        ("A_w1", 6.0, 6.7), ("A_w2", 6.7, 7.4), ("A_w3", 7.4, 8.1), ("A_w4", 8.1, 8.8),
    ];
    let mut cal: Vec<(&str, Vec<f32>)> = Vec::new();
    for (name, a, b) in &cal_windows {
        if let Some(e) = extractor.extract_embedding(&slice(*a, *b), 16_000) {
            cal.push((name, e));
        }
    }
    let sp0_cent = &centroids[&0u32];
    let sp1_cent = &centroids[&1u32];
    for (name, e) in &cal {
        eprintln!("  {}: sp0={:.3} sp1={:.3}", name, cosine(e, sp0_cent), cosine(e, sp1_cent));
    }


    // Window-length sweep: find TitaNet's minimum reliable evidence window.
    eprintln!("DIAG: --- window-length sweep (known Sp0 audio) ---");
    for len in [0.7f64, 1.0, 1.5, 2.0, 3.0, 4.5] {
        let mut row = Vec::new();
        let n = ((20.1 - 16.25) / len).floor() as usize;
        for k in 0..n {
            let a = 16.25 + len * k as f64;
            if let Some(e) = extractor.extract_embedding(&slice(a, a + len), 16_000) {
                row.push(format!("{:.2}", cosine(&e, sp0_cent)));
            }
        }
        eprintln!("  {:.1}s slices of D vs sp0 centroid: {}", len, row.join(" "));
    }

    // Dump for offline analysis.
    let dump = serde_json::json!({
        "threshold": THRESHOLD,
        "centroids": cids.iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        "segments": final_segments.iter().map(|s| serde_json::json!({
            "start": s.start_seconds, "end": s.end_seconds, "speaker": s.speaker_id,
        })).collect::<Vec<_>>(),
        "margins": margins.iter().map(|(s, e, sp, m)| serde_json::json!({
            "start": s, "end": e, "speaker": sp, "margin": m,
        })).collect::<Vec<_>>(),
        "probes": probe_emb.iter().map(|(n, _)| serde_json::json!(n)).collect::<Vec<_>>(),
    });
    let out_path = std::env::var("TEMP").unwrap() + "/cde5c264_diag.json";
    std::fs::write(&out_path, serde_json::to_string_pretty(&dump).unwrap()).expect("write dump");
    eprintln!("DIAG: dump written to {}", out_path);
}
