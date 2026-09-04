//! Success-path orchestration (change `hybrid-diarization-engine`, task 4.1):
//! ONE full-meeting pyannote pass feeds the run-assembly engine and the
//! result is the final turn set. All decisions are pure functions in
//! `run_assembly`; this module owns only model I/O and the piece/embedding
//! glue. The chunk-grid/adapter path is NOT invoked here (design D5).

use crate::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use crate::audio::speaker::pyannote_segmentation::{
    local_labels, FrameMassesOutput, PyannoteSegmentation, FRAME_SHIFT,
};
use crate::audio::speaker::run_assembly::{
    cluster_pieces, derive_pieces, drop_textless, embed_slice, margin_to_centroids, merge_to_cap,
    overlap_fraction, refine_to_centroids, resolve_turns, speech_runs, cosine, PieceIn, PieceSpan,
    AMBIGUITY_MARGIN, EMBED_FLOOR_SECS, MODE_FILTER_RADIUS_FRAMES, PIECE_CAP,
    PROMOTION_FLOOR_SECS, SPEECH_GATE, TEXT_SKEW_TOLERANCE_SECS,
};
use anyhow::Result;
use std::collections::HashMap;

pub const SAMPLE_RATE: u32 = 16_000;

/// One derived turn — the persisted unit on the success path.
#[derive(Clone, Debug)]
pub struct EngineTurn {
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub speaker_id: u32,
    pub continues_previous: bool,
    pub low_confidence: bool,
    pub overlap_frac: f32,
}

pub struct EngineOutput {
    pub turns: Vec<EngineTurn>,
    /// Final per-cluster centroids (pruned to refined membership), keyed by
    /// the cluster index used as speaker_id — the stamped-pool input.
    pub centroids: HashMap<u32, Vec<f32>>,
}

/// The persisted continuation fact (spec hard invariant): true when the
/// engine derived a continuation OR the turn's first text begins
/// mid-sentence. The machine does the suspecting — a lowercase-initial turn
/// is marked, never presented as a fresh start. Production stamping and the
/// fixture gate MUST use this same rule.
pub fn effective_continuation(engine_flag: bool, first_text: &str) -> bool {
    engine_flag || crate::audio::speaker::run_assembly::is_mid_sentence_start(first_text)
}

/// Derive speaker turns from the meeting audio. `text_spans` are the
/// transcript rows' (start, end) times — used ONLY for textless-run
/// detection, never for boundaries (spec: transcript rows are for text
/// alignment and textless detection only).
pub fn derive_turns(
    samples: &[f32],
    pya: &PyannoteSegmentation,
    extractor: &NemoEmbeddingExtractor,
    text_spans: &[(f64, f64)],
    merge_threshold: f32,
    max_speakers: usize,
) -> Result<EngineOutput> {
    let fm = pya.frame_masses(samples)?;
    derive_turns_from_masses(&fm, extractor, samples, text_spans, merge_threshold, max_speakers)
}

/// Assembly from a (possibly cached) `frame_masses` output — the exact same
/// derivation `derive_turns` performs after its single pyannote pass.
pub fn derive_turns_from_masses(
    fm: &FrameMassesOutput,
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    text_spans: &[(f64, f64)],
    merge_threshold: f32,
    max_speakers: usize,
) -> Result<EngineOutput> {
    // Layer 0: one full-meeting pass, production geometry (zero-padded tail).
    let labels = local_labels(&fm.frames, SPEECH_GATE);
    let runs = speech_runs(&fm.frames, FRAME_SHIFT);
    let pieces = derive_pieces(
        &labels,
        &runs,
        &fm.window_label_tracks,
        FRAME_SHIFT,
        MODE_FILTER_RADIUS_FRAMES,
    );
    let kept_all = drop_textless(&pieces, text_spans, TEXT_SKEW_TOLERANCE_SECS);
    // Shed-to-cap: positional, permanent (spec) — bounds clustering cost.
    let kept: &[PieceSpan] = if kept_all.len() > PIECE_CAP {
        &kept_all[..PIECE_CAP]
    } else {
        &kept_all
    };

    // Embed every piece (sub-floor included — its embedding only arbitrates
    // attachment vs promotion, never labeling).
    let sr = SAMPLE_RATE as f64;
    let mut embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(kept.len());
    for piece in kept {
        let dur = piece.end_secs - piece.start_secs;
        let (off_a, off_b) = embed_slice(dur);
        let i0 = ((piece.start_secs + off_a) * sr) as usize;
        let i1 = (((piece.start_secs + off_b) * sr) as usize).min(samples.len());
        let emb = if i1 > i0 {
            extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE)
        } else {
            None
        };
        embeddings.push(emb);
    }

    // Labeling set: pieces at/above the floor with a usable embedding.
    let labeled_idx: Vec<usize> = (0..kept.len())
        .filter(|&i| {
            embeddings[i].is_some() && kept[i].end_secs - kept[i].start_secs >= EMBED_FLOOR_SECS
        })
        .collect();
    let labeled_embs: Vec<Vec<f32>> = labeled_idx
        .iter()
        .map(|&i| embeddings[i].clone().expect("labeled piece has embedding"))
        .collect();

    let mut engine_turns = Vec::new();
    let mut centroid_map: HashMap<u32, Vec<f32>> = HashMap::new();
    if !labeled_embs.is_empty() {
        let (mut assign, mut centroids) = cluster_pieces(&labeled_embs, merge_threshold);
        merge_to_cap(&mut assign, &mut centroids, &labeled_embs, max_speakers);
        let refined = refine_to_centroids(&labeled_embs, &centroids);

        // Phantom-centroid invariant: relabel through the pruned centroid set
        // (no centroid without a refined member survives).
        let mut remap: HashMap<usize, usize> = HashMap::new();
        let mut used: Vec<Vec<f32>> = Vec::new();
        for &c in &refined {
            if !remap.contains_key(&c) {
                remap.insert(c, used.len());
                used.push(centroids[c].clone());
            }
        }
        let refined: Vec<usize> = refined.iter().map(|c| remap[c]).collect();

        // Piece inputs in time order. Labeled pieces first; then sub-floor
        // pieces resolve by identity strength and neighbor vote (engine D3
        // amendment, fixture-calibrated):
        //   - ≥ PROMOTION_FLOOR_SECS with clear margin: own turn.
        //   - ≥ PROMOTION_FLOOR_SECS, undecided identity: own turn when the
        //     neighbors do not BOTH match its best guess (an undecided slice
        //     must not be silently absorbed — boundary truth first); join
        //     only when the best guess AGREES with the agreeing neighbors.
        //   - < PROMOTION_FLOOR_SECS: join agreeing neighbors (burst
        //     fragmentation of one speaker's speech), own turn in an
        //     interjection sandwich (neighbors differ), else attach backward.
        let lab_of: HashMap<usize, usize> = labeled_idx
            .iter()
            .enumerate()
            .map(|(k, &i)| (i, k))
            .collect();

        let best_centroid = |e: &[f32]| -> Option<(usize, f32)> {
            let mut sims: Vec<(usize, f32)> =
                used.iter().enumerate().map(|(ci, c)| (ci, cosine(c, e))).collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sims.first().map(|s| (s.0, s.1)).and_then(|(ci, best)| {
                sims.get(1).map(|(_, second)| (ci, best - second))
            })
        };

        let mut resolved: Vec<Option<(usize, f32, bool)>> = vec![None; kept.len()];
        for &i in &labeled_idx {
            let k = lab_of[&i];
            resolved[i] = Some((refined[k], margin_to_centroids(&labeled_embs[k], &used), false));
        }
        // Pass 1: clear-identity promotion.
        for (i, piece) in kept.iter().enumerate() {
            if resolved[i].is_some() {
                continue;
            }
            let dur = piece.end_secs - piece.start_secs;
            if dur < EMBED_FLOOR_SECS {
                if let Some(e) = &embeddings[i] {
                    if let Some((c, m)) = best_centroid(e) {
                        if m >= AMBIGUITY_MARGIN && dur >= PROMOTION_FLOOR_SECS {
                            resolved[i] = Some((c, m, true));
                        }
                    }
                }
            }
        }
        // Pass 2: neighbor-vote resolution of the rest.
        for (i, piece) in kept.iter().enumerate() {
            if resolved[i].is_some() {
                continue;
            }
            let dur = piece.end_secs - piece.start_secs;
            if dur >= EMBED_FLOOR_SECS {
                continue;
            }
            let own = embeddings[i].as_ref().and_then(|e| best_centroid(e));
            let prev_l = (0..i).rev().find_map(|j| resolved[j].map(|(c, _, _)| c));
            let next_l = ((i + 1)..kept.len()).find_map(|j| resolved[j].map(|(c, _, _)| c));
            let neighbors_agree = matches!((prev_l, next_l), (Some(p), Some(n)) if p == n);
            // Confident agreement: own best is DECIDED (margin ≥ AMBIG) and
            // equals the agreeing neighbors. An undecided best (m < AMBIG)
            // must not read as agreement for ≥-floor pieces — that is how a
            // real interjection gets silently absorbed.
            let joins_neighbors = neighbors_agree
                && match (own, prev_l) {
                    (Some((c, m)), Some(p)) => c == p && m >= AMBIGUITY_MARGIN,
                    _ => false,
                };
            if dur >= PROMOTION_FLOOR_SECS && !joins_neighbors {
                // Long enough to be real speech without a decided identity:
                // keep the boundary, best-effort label, flagged low-confidence.
                let label = own.map(|(c, _)| c).or(prev_l).unwrap_or(0);
                resolved[i] = Some((label, 0.0, true));
            } else if neighbors_agree {
                // Burst fragmentation of one speaker's speech: join them.
                resolved[i] = Some((prev_l.expect("checked"), AMBIGUITY_MARGIN, false));
            } else if matches!((prev_l, next_l), (Some(_), Some(_))) {
                // Interjection sandwich: keep the boundary, best-effort
                // identity, flagged low-confidence.
                let label = own.map(|(c, _)| c).or(prev_l).unwrap_or(0);
                resolved[i] = Some((label, 0.0, true));
            }
            // else: attach backward (resolve_turns default).
        }

        let mut piece_ins = Vec::with_capacity(kept.len());
        for (i, piece) in kept.iter().enumerate() {
            let dur = piece.end_secs - piece.start_secs;
            match resolved[i] {
                Some((c, m, promoted)) => {
                    piece_ins.push(PieceIn {
                        start_secs: piece.start_secs,
                        dur_secs: dur,
                        cluster: Some(c),
                        margin: Some(m),
                        promoted_subfloor: promoted,
                    });
                }
                None => {
                    piece_ins.push(PieceIn {
                        start_secs: piece.start_secs,
                        dur_secs: dur,
                        cluster: None,
                        margin: None,
                        promoted_subfloor: false,
                    });
                }
            }
        }

        let turns = resolve_turns(&piece_ins);
        if std::env::var_os("MEETIFY_ENGINE_DEBUG").is_some() {
            for (p, pi) in kept.iter().zip(piece_ins.iter()) {
                eprintln!(
                    "PIECE {:9.2}-{:.2} dur={:5.2} {}",
                    p.start_secs,
                    p.end_secs,
                    pi.dur_secs,
                    match (pi.cluster, pi.margin) {
                        (Some(c), Some(m)) if pi.promoted_subfloor => {
                            format!("PROMOTED sp{c} margin={m:.3}")
                        }
                        (Some(c), Some(m)) => format!("labeled sp{c} margin={m:.3}"),
                        _ => "attached/sub-floor".to_string(),
                    }
                );
            }
        }
        for t in turns {
            let f0 = (t.start_secs / FRAME_SHIFT).floor().max(0.0) as usize;
            let f1 = ((t.end_secs / FRAME_SHIFT).ceil() as usize).min(fm.frames.len());
            let frac = if f1 > f0 { overlap_fraction(&fm.frames[f0..f1]) } else { 0.0 };
            engine_turns.push(EngineTurn {
                start_seconds: t.start_secs,
                end_seconds: t.end_secs,
                speaker_id: t.cluster as u32,
                continues_previous: t.continues_previous,
                low_confidence: t.low_confidence,
                overlap_frac: frac,
            });
        }
        centroid_map = used
            .into_iter()
            .enumerate()
            .map(|(i, c)| (i as u32, c))
            .collect();
    }

    Ok(EngineOutput {
        turns: engine_turns,
        centroids: centroid_map,
    })
}
