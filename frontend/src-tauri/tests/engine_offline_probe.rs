//! Offline engine-iteration probe (both-bars lane, 2026-09-09).
//!
//! WHY: engine-accuracy iterations must not re-pay the ~6.5-min decode and
//! the ~16-min pyannote pass. This probe loads the gate's frame-mass cache
//! (`gate_frame_masses.json`, provenance-checked — identical to re-running
//! inference with the same models and geometry) and runs the PURE stage of
//! the run-assembly engine (speech runs → corroborated pieces) with full
//! diagnostics at the ear-pin windows. No model load, no audio decode — the
//! whole run is cache-load + pure functions (~seconds).
//!
//! Pins (the user's ear):
//!   - Pin B "Yeah" at 12.0s (tolerance ±0.75s): Cynthia's back-channel
//!     inside the held sp0 turn [9.38, 13.03]. THE live accuracy failure —
//!     the engine finds no split there, so her "Yeah" renders under Carlos'
//!     badge.
//!   - Pin A Ricardo window [32.65, 41.98]: Cynthia asks "Where is
//!     Ricardo?", Carlos answers "I don't know". Engine boundaries at 36.08
//!     and 39.00/41.98 are mid-sentence engine errors absorbed by whole-atom
//!     majority at the text layer — correct rows, wrong engine internals.
//!     The diagnostics here show what the window tracks actually said.
//!
//! For each pin window this prints: the covering speech run(s), the raw
//! label-change and trust-zone slot-change candidates (with the exact guard
//! that rejected each — corroboration vs proximity), and the raw per-frame
//! pyannote masses, so the failure point is visible without any model.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::pyannote_segmentation::{local_labels, FrameMassesOutput, FRAME_SHIFT};
use app_lib::audio::speaker::run_assembly::{
    corroborate_slot_change, corroborate_split, derive_pieces, label_change_candidates,
    slot_change_candidates, speech_runs, trust_zone_track, window_slot_events,
    MODE_FILTER_RADIUS_FRAMES, SPEECH_GATE, SPLIT_TOLERANCE_SECS, TEXT_SKEW_TOLERANCE_SECS,
    TRUST_EDGE_SECS,
};

const AUDIO: &str =
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";

fn load_frame_masses() -> FrameMassesOutput {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        // Keep `cargo test` (non-ignored sweep) a no-op.
        return FrameMassesOutput {
            frames: Vec::new(),
            window_label_tracks: Vec::new(),
        };
    }
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = format!("{home}/{AUDIO}");
    let cache_path = std::path::Path::new(&audio_path)
        .parent()
        .unwrap()
        .join("gate_frame_masses.json");
    let models_dir = format!("{home}/.meetily-models");
    let prov = app_lib::audio::speaker::pyannote_segmentation::cache_provenance(
        std::path::Path::new(&format!("{models_dir}/pyannote-segmentation.onnx")),
    )
    .expect("cache provenance (model file)");
    let t0 = std::time::Instant::now();
    let fm = FrameMassesOutput::load(&cache_path, &prov)
        .expect("frame-mass cache load (run the gate once to create it)");
    eprintln!(
        "PROBE: frame masses loaded from cache in {:.1}s ({} frames = {:.1}s)",
        t0.elapsed().as_secs_f64(),
        fm.frames.len(),
        fm.frames.len() as f64 * FRAME_SHIFT
    );
    fm
}

fn secs(frame: usize) -> f64 {
    frame as f64 * FRAME_SHIFT
}

/// Diagnostics for one pin: covering runs, candidates, guard verdicts.
fn diagnose_pin(fm: &FrameMassesOutput, pin: f64, lo: f64, hi: f64, name: &str) {
    eprintln!("\n=================== PIN {name} @ {pin:.2}s (window {lo:.2}-{hi:.2}) ===================");
    let labels = local_labels(&fm.frames, SPEECH_GATE);
    let runs = speech_runs(&fm.frames, FRAME_SHIFT);
    let radius = MODE_FILTER_RADIUS_FRAMES;

    let covering: Vec<&app_lib::audio::speaker::run_assembly::SpeechRun> = runs
        .iter()
        .filter(|r| secs(r.end_frame) > lo && secs(r.start_frame) < hi)
        .collect();
    eprintln!(
        "  covering speech run(s): {}",
        covering
            .iter()
            .map(|r| format!("[{:.2},{:.2}]", secs(r.start_frame), secs(r.end_frame)))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let pieces = derive_pieces(&labels, &runs, &fm.window_label_tracks, FRAME_SHIFT, radius);
    let pin_pieces: Vec<&app_lib::audio::speaker::run_assembly::PieceSpan> = pieces
        .iter()
        .filter(|p| p.end_secs > lo && p.start_secs < hi)
        .collect();
    eprintln!(
        "  pieces in window: {}",
        pin_pieces
            .iter()
            .map(|p| format!("[{:.2},{:.2}]", p.start_secs, p.end_secs))
            .collect::<Vec<_>>()
            .join(" ")
    );

    for run in &covering {
        let run_lo = secs(run.start_frame);
        let run_hi = secs(run.end_frame);
        eprintln!("  --- run [{run_lo:.2},{run_hi:.2}) ---");

        // Merged-track label-change candidates near the pin.
        let cands = label_change_candidates(&labels, run, radius);
        let near: Vec<f64> = cands
            .iter()
            .map(|&c| secs(c))
            .filter(|t| *t >= lo - 1.5 && *t <= hi + 1.5)
            .collect();
        eprintln!("  label-change candidates near pin: {:?}", near);

        // Trust-zone slot-change candidates near the pin.
        let tz = trust_zone_track(&fm.window_label_tracks, fm.frames.len(), FRAME_SHIFT);
        let tz_cands = slot_change_candidates(&tz, run, radius);
        let tz_near: Vec<f64> = tz_cands
            .iter()
            .map(|&c| secs(c))
            .filter(|t| *t >= lo - 1.5 && *t <= hi + 1.5)
            .collect();
        eprintln!("  trust-zone slot-change candidates near pin: {:?}", tz_near);

        // Guard verdicts at every candidate time.
        let window_events = window_slot_events(&fm.window_label_tracks, FRAME_SHIFT, radius);
        for t in near.iter().chain(tz_near.iter()) {
            let two_window = corroborate_split(
                &fm.window_label_tracks,
                *t,
                FRAME_SHIFT,
                SPLIT_TOLERANCE_SECS,
                radius,
            );
            let trust = corroborate_slot_change(&window_events, *t, SPLIT_TOLERANCE_SECS);
            // Proximity: within tolerance of an accepted bound or the run edge?
            let end_t = secs(run.end_frame);
            let near_accepted = pieces.iter().any(|p| {
                ((p.start_secs - t).abs() <= SPLIT_TOLERANCE_SECS
                    || (p.end_secs - t).abs() <= SPLIT_TOLERANCE_SECS)
            }) || (end_t - t).abs() <= SPLIT_TOLERANCE_SECS;
            eprintln!(
                "  cand {t:.3}: corroborate_split={two_window} corroborate_slot={trust} near_accepted_bound={near_accepted}"
            );
        }

        // Which windows' trust zones cover the pin?
        for (i, w) in window_events.iter().enumerate() {
            if pin >= w.trust_lo && pin < w.trust_hi {
                let events_near: Vec<f64> = w
                    .events
                    .iter()
                    .filter(|e| (**e - pin).abs() <= 1.0)
                    .copied()
                    .collect();
                eprintln!(
                    "  window #{i} trust zone covers pin (events within 1s: {:?})",
                    events_near
                );
            }
        }
        let _ = TRUST_EDGE_SECS; // referenced for context in output above
    }

    // Raw per-frame masses across the window, every 5 frames (~85ms).
    eprintln!("  raw masses (sp0 sp1 sp2 | overlap silence):");
    let f0 = ((lo / FRAME_SHIFT) as usize).saturating_sub(2);
    let f1 = ((hi / FRAME_SHIFT) as usize + 2).min(fm.frames.len());
    let mut i = f0;
    while i < f1 {
        let fp = &fm.frames[i];
        eprintln!(
            "    {:8.3}  {:.2} {:.2} {:.2} | {:.2} {:.2}",
            secs(i),
            fp.speaker[0],
            fp.speaker[1],
            fp.speaker[2],
            fp.overlap,
            fp.silence
        );
        i += 5;
    }
}

#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe -- --ignored --nocapture"]
async fn boundary_diagnostics_at_ear_pins() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let fm = load_frame_masses();

    // Pin B: Cynthia's "Yeah" inside the held sp0 turn.
    diagnose_pin(&fm, 12.0, 9.38, 13.03, "B (Yeah, 12±0.75)");
    // Pin A: the Ricardo question/answer window.
    diagnose_pin(&fm, 36.08, 32.65, 41.98, "A (Where is Ricardo / I don't know)");

    // Meeting-wide piece census for context.
    let labels = local_labels(&fm.frames, SPEECH_GATE);
    let runs = speech_runs(&fm.frames, FRAME_SHIFT);
    let pieces = derive_pieces(
        &labels,
        &runs,
        &fm.window_label_tracks,
        FRAME_SHIFT,
        MODE_FILTER_RADIUS_FRAMES,
    );
    let kept = app_lib::audio::speaker::run_assembly::drop_textless(
        &pieces,
        // transcript spans are only needed for the textless filter; pull them
        // from transcripts.json beside the audio (same as the gate).
        &transcript_spans(),
        TEXT_SKEW_TOLERANCE_SECS,
    );
    eprintln!(
        "\nPROBE: meeting-wide: {} speech runs, {} pieces, {} text-bearing pieces",
        runs.len(),
        pieces.len(),
        kept.len()
    );
}

/// Cached 16kHz mono f32 samples beside the audio (the decode costs ~6.5
/// min; the both-bars iteration loop must not re-pay it). Format: raw
/// little-endian f32, sample count in `samples_16k.meta.json`.
fn load_samples() -> Vec<f32> {
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = format!("{home}/{AUDIO}");
    let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
    let cache = dir.join("samples_16k.f32");
    let meta = dir.join("samples_16k.meta.json");
    if cache.exists() && meta.exists() {
        let bytes = std::fs::read(&cache).expect("read samples cache");
        let n: usize = serde_json::from_str(
            &std::fs::read_to_string(&meta).expect("read samples meta"),
        )
        .expect("parse samples meta");
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(samples.len(), n, "samples cache/meta mismatch");
        eprintln!("PROBE: samples loaded from cache ({} = {:.1}s)", n, n as f64 / 16_000.0);
        return samples;
    }
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    let bytes: Vec<u8> = samples
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    std::fs::write(&cache, &bytes).expect("write samples cache");
    std::fs::write(&meta, samples.len().to_string()).expect("write samples meta");
    eprintln!(
        "PROBE: decoded {:.1}s and cached {} samples to {}",
        decoded.duration_seconds,
        samples.len(),
        cache.display()
    );
    samples
}

/// Can TitaNet see the 12.0s voice flip that pyannote-segmentation missed?
/// Prints (a) a same-voice baseline (adjacent windows inside the held Carlos
/// run), (b) direct Carlos-vs-"Yeah"-core similarities, and (c) an
/// adjacent-window cosine dip scan t = 10.5..13.5s. A voice change at t
/// shows as a local similarity minimum below the same-voice baseline.
#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe embedding_separation_at_pin_b -- --ignored --nocapture"]
async fn embedding_separation_at_pin_b() {
    use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples = load_samples();
    let home = std::env::var("USERPROFILE").unwrap();
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{home}/.meetily-models/{}",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");
    const SR: usize = 16_000;

    let emb = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = ((a * SR as f64) as usize).min(samples.len());
        let i1 = ((b * SR as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SR as u32)
    };
    let cos = |a: &Option<Vec<f32>>, b: &Option<Vec<f32>>| -> f32 {
        match (a, b) {
            (Some(x), Some(y)) => app_lib::audio::speaker::run_assembly::cosine(x, y),
            _ => f32::NAN,
        }
    };

    // (a) Same-voice baseline: adjacent 1.2s windows inside Carlos' decode.
    let base1 = cos(&emb(9.5, 10.7), &emb(10.7, 11.9));
    let base2 = cos(&emb(10.0, 11.2), &emb(11.2, 11.9));
    eprintln!("BASELINE same-voice adjacent cosines: {base1:.3} {base2:.3}");

    // (b) Carlos anchor vs the Yeah core (11.95-12.75) and its surround.
    let carlos = emb(9.5, 11.9);
    let yeah = emb(11.95, 12.75);
    let after = emb(12.75, 13.9);
    eprintln!(
        "cos(carlos 9.5-11.9, yeah-core 11.95-12.75) = {:.3}",
        cos(&carlos, &yeah)
    );
    eprintln!(
        "cos(carlos 9.5-11.9, after 12.75-13.9)      = {:.3}",
        cos(&carlos, &after)
    );
    eprintln!("cos(yeah-core, after)                       = {:.3}", cos(&yeah, &after));

    // (c) Dip scan: adjacent 1.2s windows stepping 0.1s.
    let mut t = 10.5f64;
    while t <= 13.5 {
        let c = cos(&emb(t - 1.2, t), &emb(t, t + 1.2));
        eprintln!("SCAN t={t:5.2}  cos(left,right) = {c:.3}");
        t += 0.1;
    }
}

/// Census of pyannote "confusion valleys" meeting-wide — the candidate set
/// for the sub-run voice-flip scan. A valley frame sits inside a speech run
/// and is contested: argmax speaker mass < 0.7 while another speaker's mass
/// ≥ 0.15 (the 12.03-12.71 signature at pin B). Prints valley count, total
/// valley seconds (≈ production embedding cost driver), and every valley in
/// the two pin windows.
#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe valley_census -- --ignored --nocapture"]
async fn valley_census() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let fm = load_frame_masses();
    let runs = speech_runs(&fm.frames, FRAME_SHIFT);

    let contested = |fp: &app_lib::audio::speaker::pyannote_segmentation::FrameMasses| {
        let mut s = [fp.speaker[0], fp.speaker[1], fp.speaker[2]];
        s.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        s[0] < 0.7 && s[1] >= 0.15
    };
    let mut in_run = vec![false; fm.frames.len()];
    for r in &runs {
        for f in r.start_frame..r.end_frame.min(fm.frames.len()) {
            in_run[f] = true;
        }
    }
    let mut valleys: Vec<(usize, usize)> = Vec::new();
    let mut total_frames = 0usize;
    for f in 0..fm.frames.len() {
        let is_v = in_run[f] && contested(&fm.frames[f]);
        if is_v {
            total_frames += 1;
        }
        match valleys.last_mut() {
            Some(last) if is_v && last.1 == f => last.1 = f + 1,
            _ if is_v => valleys.push((f, f + 1)),
            _ => {}
        }
    }
    // Merge valleys separated by < 10 frames (~0.17s).
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for v in valleys {
        match merged.last_mut() {
            Some(last) if v.0 - last.1 < 10 => last.1 = v.1,
            _ => merged.push(v),
        }
    }
    eprintln!(
        "VALLEYS: {} valley spans, {} frames total ({:.1}s contested inside runs)",
        merged.len(),
        total_frames,
        total_frames as f64 * FRAME_SHIFT
    );
    for (a, b) in &merged {
        let t0 = secs(*a);
        let t1 = secs(*b);
        if (t0 >= 9.0 && t0 <= 14.5) || (t0 >= 32.0 && t0 <= 43.0) {
            eprintln!("  VALLEY (pin window) [{t0:.3},{t1:.3}]");
        }
    }
    let near_pins = merged
        .iter()
        .filter(|(a, _)| {
            let t = secs(*a);
            (11.0..=13.0).contains(&t) || (32.0..=42.0).contains(&t)
        })
        .count();
    eprintln!("VALLEYS: {near_pins} valley span(s) start within the pin windows");
}

/// Who speaks the Ricardo-question pieces? Embeds the engine's piece spans
/// in 32-40s against identity anchors picked from the PERSISTED, ear-verified
/// rows: Cynthia anchor = [9.75,11.8] ("...aged like five years", S1) and
/// Carlos anchor = [12.0,12.8] (the user's "Yeah", S2) + [22.91,28.5]
/// (Speaker 1 "So for search..." stretch).
#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe ricardo_window_identity -- --ignored --nocapture"]
async fn ricardo_window_identity() {
    use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples = load_samples();
    let home = std::env::var("USERPROFILE").unwrap();
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{home}/.meetily-models/{}",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");
    const SR: usize = 16_000;
    let emb = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = ((a * SR as f64) as usize).min(samples.len());
        let i1 = ((b * SR as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SR as u32)
    };
    let cos = |a: &Option<Vec<f32>>, b: &Option<Vec<f32>>| -> f32 {
        match (a, b) {
            (Some(x), Some(y)) => app_lib::audio::speaker::run_assembly::cosine(x, y),
            _ => f32::NAN,
        }
    };

    let cynthia = emb(9.75, 11.8);
    let carlos_a = emb(12.0, 12.8);
    let carlos_b = emb(22.91, 28.5);
    eprintln!(
        "anchor cross-check cos(cynthia, carlos_a)={:.3} cos(cynthia, carlos_b)={:.3} cos(carlos_a, carlos_b)={:.3}",
        cos(&cynthia, &carlos_a),
        cos(&cynthia, &carlos_b),
        cos(&carlos_a, &carlos_b)
    );

    let probes: &[(&str, f64, f64)] = &[
        ("run [32.65,32.97]", 32.65, 32.97),
        ("piece [34.29,34.66]", 34.29, 34.66),
        ("piece [34.66,36.08]", 34.66, 36.08),
        ("piece [36.08,38.64]", 36.08, 38.64),
        ("run   [39.00,39.93]", 39.00, 39.93),
        ("piece [41.98,45.0] ", 41.98, 45.0),
    ];
    for (name, a, b) in probes {
        let e = emb(*a, *b);
        eprintln!(
            "{name}: cos(cynthia)={:.3} cos(carlos_a)={:.3} cos(carlos_b)={:.3}",
            cos(&e, &cynthia),
            cos(&e, &carlos_a),
            cos(&e, &carlos_b)
        );
    }
}

/// Three-voice badge-accuracy probe. Anchors are ear-attested: Cynthia
/// [9.75,11.8] (S1), Carlos [12.0,12.8] (S2 "Yeah") + [42.2,50.0] ("I saw
/// you there..."), Ricardo [2810,2818] (inside S11's attested single-voice
/// stretch). Probes: the Ricardo stretch sub-spans (is it conflated into a
/// cluster?), the fused 54.5-80s row, the "Let me ping in"/"I can't" window,
/// and the textless Ricardo join at 1057s.
#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test engine_offline_probe badge_accuracy_probe -- --ignored --nocapture"]
async fn badge_accuracy_probe() {
    use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples = load_samples();
    let home = std::env::var("USERPROFILE").unwrap();
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{home}/.meetily-models/{}",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");
    const SR: usize = 16_000;
    let emb = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = ((a * SR as f64) as usize).min(samples.len());
        let i1 = ((b * SR as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SR as u32)
    };
    let cos = |a: &Option<Vec<f32>>, b: &Option<Vec<f32>>| -> f32 {
        match (a, b) {
            (Some(x), Some(y)) => app_lib::audio::speaker::run_assembly::cosine(x, y),
            _ => f32::NAN,
        }
    };

    let cynthia = emb(9.75, 11.8);
    let carlos_a = emb(12.0, 12.8);
    let carlos_b = emb(42.2, 50.0);
    let ricardo = emb(2810.0, 2818.0);
    eprintln!(
        "anchors: cy~ca={:.3} cy~cb={:.3} cy~ri={:.3} ca~cb={:.3} ca~ri={:.3} cb~ri={:.3}",
        cos(&cynthia, &carlos_a),
        cos(&cynthia, &carlos_b),
        cos(&cynthia, &ricardo),
        cos(&carlos_a, &carlos_b),
        cos(&carlos_a, &ricardo),
        cos(&carlos_b, &ricardo)
    );

    let probes: &[(&str, f64, f64)] = &[
        ("Ricardo join 17:37     ", 1057.0, 1062.0),
        ("Ricardo join 17:37 wide", 1057.0, 1077.0),
        ("Ricardo stretch a      ", 2801.3, 2805.0),
        ("Ricardo stretch b      ", 2805.0, 2812.0),
        ("Ricardo stretch c      ", 2812.0, 2818.0),
        ("Ricardo stretch d      ", 2818.0, 2821.0),
        ("fused row [54.5,60]    ", 54.5, 60.0),
        ("fused row [60,65]      ", 60.0, 65.0),
        ("fused row [65,70]      ", 65.0, 70.0),
        ("fused row [70,75]      ", 70.0, 75.0),
        ("fused row [75,80]      ", 75.0, 80.0),
        ("ping-in [36.4,38.6]    ", 36.4, 38.6),
        ("I-cant  [38.9,40.2]    ", 38.9, 40.2),
        ("saw-you [42.2,45.0]    ", 42.2, 45.0),
    ];
    for (name, a, b) in probes {
        let e = emb(*a, *b);
        eprintln!(
            "{name}: cy={:.3} ca={:.3} cb={:.3} ri={:.3}",
            cos(&e, &cynthia),
            cos(&e, &carlos_a),
            cos(&e, &carlos_b),
            cos(&e, &ricardo)
        );
    }

    // MEETIFY_REFS_OUT=path: write ear-attested anchor-mean embeddings as
    // the enrollment seeds (voice fingerprinting). Anchors: Cynthia =
    // [9.75,11.8] (S1) + [35.0,36.0] (inside the Cynthia-attested Ricardo
    // question piece); Carlos = [12.0,12.8] (S2 "Yeah") + [42.2,50.0]
    // (pre-Ricardo-join, far from Cynthia — Carlos by elimination); Ricardo
    // is written too (for reference) but the engine takes max_speakers-1
    // refs and his pieces already self-cluster decisively.
    if let Some(path) = std::env::var_os("MEETIFY_REFS_OUT") {
        let mean_norm = |spans: &[(f64, f64)]| -> Option<Vec<f32>> {
            let mut acc = vec![0.0f32; carlos_a.as_ref().unwrap().len()];
            let mut n = 0usize;
            for (a, b) in spans {
                if let Some(e) = emb(*a, *b) {
                    for (d, v) in acc.iter_mut().zip(&e) {
                        *d += v;
                    }
                    n += 1;
                }
            }
            if n == 0 {
                return None;
            }
            let norm = acc.iter().map(|x| x * x).sum::<f32>().sqrt();
            for v in acc.iter_mut() {
                *v /= norm;
            }
            Some(acc)
        };
        let refs = serde_json::json!({
            "Cynthia": mean_norm(&[(9.75, 11.8), (35.0, 36.0)]),
            "Carlos": mean_norm(&[(12.0, 12.8), (42.2, 50.0)]),
            "Ricardo": mean_norm(&[(2805.0, 2812.0), (2812.0, 2818.0)]),
        });
        std::fs::write(&path, serde_json::to_string_pretty(&refs).unwrap()).expect("write refs");
        eprintln!("REFS written to {}", path.to_string_lossy());
    }

    // MEETIFY_CENTROID_SCORE=path: score the anchors against the engine's
    // final centroids — settles who owns which cluster and how far each
    // voice sits from the centroid that absorbs it.
    if let Some(path) = std::env::var_os("MEETIFY_CENTROID_SCORE") {
        let cents: std::collections::BTreeMap<String, Vec<f32>> = serde_json::from_str(
            &std::fs::read_to_string(&path).expect("read centroids dump"),
        )
        .expect("parse centroids");
        for (id, c) in &cents {
            eprintln!(
                "CENTROID {id}: cos(cynthia)={:.3} cos(carlos_a)={:.3} cos(carlos_b)={:.3} cos(ricardo)={:.3}",
                app_lib::audio::speaker::run_assembly::cosine(c, cynthia.as_ref().unwrap()),
                app_lib::audio::speaker::run_assembly::cosine(c, carlos_a.as_ref().unwrap()),
                app_lib::audio::speaker::run_assembly::cosine(c, carlos_b.as_ref().unwrap()),
                app_lib::audio::speaker::run_assembly::cosine(c, ricardo.as_ref().unwrap()),
            );
        }
    }
}

fn transcript_spans() -> Vec<(f64, f64)> {
    let home = std::env::var("USERPROFILE").unwrap();
    let audio_path = format!("{home}/{AUDIO}");
    let transcript_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(&audio_path)
                .parent()
                .unwrap()
                .join("transcripts.json"),
        )
        .expect("read transcripts.json"),
    )
    .expect("parse transcripts.json");
    transcript_json
        .get("segments")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some((
                        r.get("audio_start_time")?.as_f64()?,
                        r.get("audio_end_time")?.as_f64()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}
