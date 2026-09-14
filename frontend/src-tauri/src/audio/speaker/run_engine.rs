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
    cluster_pieces_ahc, confusion_valleys, derive_pieces, drop_textless, embed_slice,
    margin_to_centroids, merge_to_cap, overlap_fraction, refine_loop, resolve_turns,
    rescue_candidates, speech_runs, cosine, PieceIn, PieceSpan, RescueGap, RescueSubWindow,
    SpeechRun, AMBIGUITY_MARGIN, EMBED_FLOOR_SECS, MODE_FILTER_RADIUS_FRAMES, PIECE_CAP,
    PROMOTION_FLOOR_SECS, SPEECH_GATE, TEXT_SKEW_TOLERANCE_SECS,
};
use anyhow::Result;
use std::collections::HashMap;

pub const SAMPLE_RATE: u32 = 16_000;

// Gap-rescue constants (change `gap-speech-voice-attribution`; calibrated on
// cde5c264 under the fixture-gate rule, 2026-09-07 — see the change's design).
/// Mirrors `commands::GAP_BORROW_MAX_MS` (kept local: commands depends on this
/// module, not vice versa).
const BORROW_CAP_MS: i64 = 3_000;
/// Minimum voiced sub-window offered for identity (0.12s — tuned so the S2b
/// "Oh, man" onset [15.64,15.82] qualifies; identity on such slices is gated
/// by the rescue margin).
const MIN_SUB_WINDOW_SECS: f64 = 0.12;
const ONSET_THRESHOLD_DB: f32 = 10.0;
const MERGE_GAP_FRAMES: usize = 4;
const SEG_HOP: usize = SAMPLE_RATE as usize / 50; // 20ms

// Sub-run voice-flip scan (both-bars, 2026-09-09; see `confusion_valleys` in
// run_assembly for the candidate detector and its measured pin-B evidence).
/// Voice-check window either side of a valley start: long enough for a
/// usable TitaNet slice, short enough that one side stays inside a short
/// back-channel's surroundings (the 12.0s "Yeah" check must fit the
/// [10.8,12.0) / [12.0,13.2) windows inside the 3.65s piece).
const FLIP_WINDOW_SECS: f64 = 1.2;
/// Pieces shorter than this cannot host a promotable split: the check needs
/// PROMOTION_FLOOR_SECS of material on both sides plus a full left window.
const FLIP_SCAN_MIN_PIECE_SECS: f64 = 2.0;
/// How far past the piece's end the right window may reach (it routinely
/// covers the run's decay into silence — the measured 12.0s right window
/// [12.0,13.2) spills 0.17s past the piece end at 13.03).
const FLIP_RIGHT_SPILL_SECS: f64 = 0.5;
/// Splice rounds: each round splits at most one accepted valley per piece;
/// newly created pieces are re-scanned next round. Bounded, deterministic.
const FLIP_MAX_ROUNDS: usize = 3;

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
    references: &[(String, Vec<f32>)],
) -> Result<EngineOutput> {
    let fm = pya.frame_masses(samples)?;
    derive_turns_from_masses(
        &fm,
        extractor,
        samples,
        text_spans,
        merge_threshold,
        max_speakers,
        references,
    )
}

/// Assembly from a (possibly cached) `frame_masses` output — the exact same
/// derivation `derive_turns` performs after its single pyannote pass.
/// `references` are enrolled voice fingerprints (named-speaker embeddings,
/// possibly empty): they anchor clustering as stable extra centroids, so
/// ambiguous pieces resolve against known voices instead of noisy
/// meeting-internal averages.
pub fn derive_turns_from_masses(
    fm: &FrameMassesOutput,
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    text_spans: &[(f64, f64)],
    merge_threshold: f32,
    max_speakers: usize,
    references: &[(String, Vec<f32>)],
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
        // Reference-anchored clustering (enrollment lever): enrolled voice
        // fingerprints enter as extra stable centroids, and the meeting's
        // piece-cluster budget shrinks accordingly — ambiguous pieces
        // resolve against known voices instead of noisy internal averages.
        let refs: Vec<&Vec<f32>> =
            references.iter().take(max_speakers.max(1) - 1).map(|(_, e)| e).collect();
        let piece_cap = (max_speakers - refs.len()).max(1);
        // Average-linkage AHC (both-bars accuracy fix, 2026-09-09): the
        // greedy pass seeded centroids in processing order and drifted — on
        // cde5c264 it left CARLOS with no centroid (his anchors scored
        // 0.12–0.32 against both surviving centroids) so his pieces flipped
        // between the Cynthia and Ricardo clusters by thin margins and
        // Ricardo's real speech shared his badge. AHC measures the embedding
        // matrix directly; the threshold keeps its meaning.
        let (mut assign, mut centroids) = cluster_pieces_ahc(&labeled_embs, merge_threshold);
        merge_to_cap(&mut assign, &mut centroids, &labeled_embs, piece_cap);
        for e in &refs {
            centroids.push((*e).clone());
        }
        // Lloyd loop instead of a single refine pass: one pass keeps pieces
        // on stale greedy centroids (the false 34.66s boundary — S7). Pieces
        // nearer a reference centroid join it here.
        refine_loop(&mut assign, &mut centroids, &labeled_embs, 10);
        let mut refined = assign;

        // Deduplicate centroids that converged onto the same voice (e.g. an
        // enrolled reference and the meeting cluster of that same person):
        // merge the higher index into the lower, reassign, recompute the
        // keepers from their members. Deterministic; bounded.
        loop {
            let mut merge: Option<(usize, usize)> = None;
            for i in 0..centroids.len() {
                for j in (i + 1)..centroids.len() {
                    if cosine(&centroids[i], &centroids[j]) >= 0.85 {
                        merge = Some((j, i));
                        break;
                    }
                }
                if merge.is_some() {
                    break;
                }
            }
            let Some((victim, keeper)) = merge else { break };
            for a in refined.iter_mut() {
                if *a == victim {
                    *a = keeper;
                } else if *a > victim {
                    *a -= 1;
                }
            }
            centroids.remove(victim);
            let dim = centroids[0].len();
            let mut sums = vec![0.0f32; centroids.len() * dim];
            let mut counts = vec![0usize; centroids.len()];
            for (k, ci) in refined.iter().enumerate() {
                counts[*ci] += 1;
                for (d, v) in labeled_embs[k].iter().enumerate() {
                    sums[*ci * dim + d] += v;
                }
            }
            for (ci, c) in centroids.iter_mut().enumerate() {
                if counts[ci] > 0 {
                    for d in 0..dim {
                        c[d] = sums[ci * dim + d] / counts[ci] as f32;
                    }
                }
            }
        }

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

        // Sub-run voice-flip scan: pyannote holds the argmax through short
        // other-voice back-channels (the 12.0s "Yeah" — decode turns to
        // contested mush, never flips, so no split candidate ever exists).
        // At each confusion valley inside a labeled piece, embed both sides
        // of the valley start and split ONLY when both sides embed
        // decisively (margin ≥ AMBIGUITY_MARGIN) to DIFFERENT final
        // centroids — the same identity bar a labeled piece must clear, so
        // an undecided or same-voice valley can never manufacture a split.
        // Each round scans the CURRENT piece list and splits at most once
        // per piece; already-split edges abstain naturally (the remaining
        // half can no longer fit both windows).
        let flip_debug = std::env::var_os("MEETIFY_ENGINE_DEBUG").is_some();
        for _round in 0..FLIP_MAX_ROUNDS {
            let mut flips: Vec<(usize, f64, usize, f32, usize, f32)> = Vec::new();
            for (i, piece) in piece_ins.iter().enumerate() {
                if piece.cluster.is_none() {
                    continue;
                }
                if piece.dur_secs < FLIP_SCAN_MIN_PIECE_SECS {
                    continue;
                }
                let p_end = piece.start_secs + piece.dur_secs;
                let f_start = (piece.start_secs / FRAME_SHIFT).round() as usize;
                let f_end = ((p_end / FRAME_SHIFT).round() as usize).min(fm.frames.len());
                let piece_run = SpeechRun {
                    start_frame: f_start,
                    end_frame: f_end,
                };
                for (va, _vb) in confusion_valleys(&fm.frames, &piece_run) {
                    let t = va as f64 * FRAME_SHIFT;
                    if t - FLIP_WINDOW_SECS < piece.start_secs
                        || t - piece.start_secs < PROMOTION_FLOOR_SECS
                        || p_end - t < PROMOTION_FLOOR_SECS
                        || t + FLIP_WINDOW_SECS > p_end + FLIP_RIGHT_SPILL_SECS
                    {
                        continue;
                    }
                    let i0 = (((t - FLIP_WINDOW_SECS) * sr) as usize).min(samples.len());
                    let i1 = ((t * sr) as usize).min(samples.len());
                    let j1 = (((t + FLIP_WINDOW_SECS) * sr) as usize).min(samples.len());
                    if i1 <= i0 || j1 <= i1 {
                        continue;
                    }
                    let (Some(emb_l), Some(emb_r)) = (
                        extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE),
                        extractor.extract_embedding(&samples[i1..j1], SAMPLE_RATE),
                    ) else {
                        continue;
                    };
                    let (lc, lm) = best_centroid(&emb_l).unwrap_or((0, f32::NEG_INFINITY));
                    let (rc, rm) = best_centroid(&emb_r).unwrap_or((1, f32::NEG_INFINITY));
                    if lm >= AMBIGUITY_MARGIN && rm >= AMBIGUITY_MARGIN && lc != rc {
                        flips.push((i, t, lc, lm, rc, rm));
                        if flip_debug {
                            eprintln!(
                                "FLIP-SPLIT piece[{:.2},{:.2}] at {:.2}: sp{} -> sp{} margins {:.3}/{:.3}",
                                piece.start_secs, p_end, t, lc, rc, lm, rm
                            );
                        }
                        break; // one split per piece per round; halves re-scanned next round
                    }
                    if flip_debug {
                        eprintln!(
                            "FLIP-ABSTAIN at {:.2}: margins {:.3}/{:.3} clusters {}/{}",
                            t, lm, rm, lc, rc
                        );
                    }
                }
            }
            if flips.is_empty() {
                break;
            }
            // Splice: replace each flipped piece with its two halves.
            // Descending index keeps earlier positions valid (each piece
            // flips at most once per round).
            flips.sort_by(|a, b| b.0.cmp(&a.0));
            for (i, t, lc, lm, rc, rm) in flips {
                let piece = piece_ins[i];
                let left = PieceIn {
                    start_secs: piece.start_secs,
                    dur_secs: t - piece.start_secs,
                    cluster: Some(lc),
                    margin: Some(lm),
                    promoted_subfloor: false,
                };
                let right = PieceIn {
                    start_secs: t,
                    dur_secs: piece.start_secs + piece.dur_secs - t,
                    cluster: Some(rc),
                    margin: Some(rm),
                    promoted_subfloor: false,
                };
                piece_ins[i] = right;
                piece_ins.insert(i, left);
            }
        }

        let turns_pre = resolve_turns(&piece_ins);
        // Gap rescue (v3, change `gap-speech-voice-attribution`): text-bearing
        // pyannote-silence gaps attributed by voice. Candidates are evaluated
        // against the PRE-rescue turn set (snapshot semantics) and spliced in
        // one time-ordered pass; the second resolve produces the final turns.
        let pre_turn_tuples: Vec<(f64, f64, u32)> = turns_pre
            .iter()
            .map(|t| (t.start_secs, t.end_secs, t.cluster as u32))
            .collect();
        let rescue_gaps =
            build_rescue_gaps(samples, &runs, text_spans, &pre_turn_tuples, &used, extractor);
        let candidates = rescue_candidates(
            &pre_turn_tuples,
            &rescue_gaps,
            PROMOTION_FLOOR_SECS,
            AMBIGUITY_MARGIN,
            BORROW_CAP_MS,
        );
        let rescue_debug = std::env::var_os("MEETIFY_ENGINE_DEBUG").is_some();
        let piece_ins_pre = rescue_debug.then(|| piece_ins.clone());
        for cand in &candidates {
            let at = piece_ins
                .iter()
                .position(|p| p.start_secs >= cand.start_secs)
                .unwrap_or(piece_ins.len());
            piece_ins.insert(
                at,
                PieceIn {
                    start_secs: cand.start_secs,
                    dur_secs: cand.end_secs - cand.start_secs,
                    cluster: Some(cand.cluster),
                    margin: Some(cand.margin),
                    promoted_subfloor: true,
                },
            );
            if rescue_debug {
                eprintln!(
                    "RESCUE sp{} [{:.2},{:.2}] margin={:.3}",
                    cand.cluster, cand.start_secs, cand.end_secs, cand.margin
                );
            }
        }
        let mut turns = turns_pre;
        if !candidates.is_empty() {
            turns = resolve_turns(&piece_ins);
        }
        if rescue_debug {
            if let Some(pre) = &piece_ins_pre {
                for (p, pi) in kept.iter().zip(pre.iter()) {
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
            for pi in piece_ins.iter().skip(kept.len()) {
                eprintln!(
                    "PIECE {:9.2}-{:.2} dur={:5.2} RESCUED sp{}",
                    pi.start_secs,
                    pi.start_secs + pi.dur_secs,
                    pi.dur_secs,
                    pi.cluster.map(|c| c.to_string()).unwrap_or_default()
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

/// Union of all transcript-row spans clipped to [a, b) (None = no overlap).
fn union_span_in(spans: &[(f64, f64)], a: f64, b: f64) -> Option<(f64, f64)> {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    let mut any = false;
    for (s, e) in spans {
        let lo_s = (*s).max(a);
        let hi_s = (*e).min(b);
        if hi_s > lo_s {
            any = true;
            lo = lo.min(lo_s);
            hi = hi.max(hi_s);
        }
    }
    if any {
        Some((lo, hi))
    } else {
        None
    }
}

/// Energy segmentation of [a, b) into voiced sub-windows: 20ms frames, RMS
/// dBFS, baseline = p25 of the span's frames (non-voiced level estimate),
/// threshold baseline + ONSET_THRESHOLD_DB, gaps shorter than
/// MERGE_GAP_FRAMES merged,
/// sub-windows below MIN_SUB_WINDOW_SECS dropped. Deterministic.
fn segment_voiced_sub_windows(samples: &[f32], a: f64, b: f64) -> Vec<(f64, f64)> {
    let i0 = (a * SAMPLE_RATE as f64) as usize;
    let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
    let mut dbs: Vec<f32> = Vec::new();
    let mut j = i0;
    while j + SEG_HOP <= i1 {
        let rms = (samples[j..j + SEG_HOP].iter().map(|v| v * v).sum::<f32>() / SEG_HOP as f32)
            .sqrt();
        dbs.push(20.0 * rms.max(1e-10).log10());
        j += SEG_HOP;
    }
    if dbs.is_empty() {
        return Vec::new();
    }
    let mut sorted = dbs.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let baseline = sorted[sorted.len() / 4];
    let thr = baseline + ONSET_THRESHOLD_DB;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut k = 0usize;
    while k < dbs.len() {
        if dbs[k] >= thr {
            let start = k;
            while k < dbs.len() && dbs[k] >= thr {
                k += 1;
            }
            runs.push((start, k));
        } else {
            k += 1;
        }
    }
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in runs {
        match merged.last_mut() {
            Some(last) if r.0 - last.1 < MERGE_GAP_FRAMES => last.1 = r.1,
            _ => merged.push(r),
        }
    }
    merged
        .iter()
        .map(|(s, e)| {
            (
                a + (*s * SEG_HOP) as f64 / SAMPLE_RATE as f64,
                a + (*e * SEG_HOP) as f64 / SAMPLE_RATE as f64,
            )
        })
        .filter(|(s, e)| e - s >= MIN_SUB_WINDOW_SECS)
        .collect()
}

/// Build the rescue inputs: text-bearing distinct-turn silence gaps with their
/// voiced sub-window identity votes (this is the engine's model I/O — slicing
/// and embedding; all gating lives in `rescue_candidates`).
fn build_rescue_gaps(
    samples: &[f32],
    runs: &[crate::audio::speaker::run_assembly::SpeechRun],
    text_spans: &[(f64, f64)],
    turns: &[(f64, f64, u32)],
    used: &[Vec<f32>],
    extractor: &NemoEmbeddingExtractor,
) -> Vec<RescueGap> {
    let mut gaps = Vec::new();
    let mut bounds: Vec<(f64, f64)> = Vec::new();
    if let Some(first) = runs.first() {
        bounds.push((0.0, first.start_frame as f64 * FRAME_SHIFT));
    }
    for w in runs.windows(2) {
        bounds.push((
            w[0].end_frame as f64 * FRAME_SHIFT,
            w[1].start_frame as f64 * FRAME_SHIFT,
        ));
    }
    if let Some(last) = runs.last() {
        bounds.push((
            last.end_frame as f64 * FRAME_SHIFT,
            samples.len() as f64 / SAMPLE_RATE as f64,
        ));
    }
    for (ga, gb) in bounds {
        if gb <= ga {
            continue;
        }
        // interior and meeting-edge gaps abstain before any model I/O
        let Some(lc) = turns.iter().filter(|(_, e, _)| *e <= ga + 1e-9).last().map(|t| t.2)
        else {
            continue;
        };
        let Some(rc) = turns.iter().find(|(s, _, _)| *s >= gb - 1e-9).map(|t| t.2) else {
            continue;
        };
        if lc == rc {
            continue;
        }
        let Some((sa, sb)) = union_span_in(text_spans, ga, gb) else {
            continue;
        };
        if sb - sa < PROMOTION_FLOOR_SECS {
            continue;
        }
        let debug = std::env::var_os("MEETIFY_ENGINE_DEBUG").is_some();
        if debug {
            eprintln!(
                "RESCUE-GAP [{ga:.2},{gb:.2}] sp{lc}->sp{rc} raw [{sa:.2},{sb:.2}] dur {:.2}",
                sb - sa
            );
        }
        let mut votes: Vec<RescueSubWindow> = Vec::new();
        for (ssa, sse) in segment_voiced_sub_windows(samples, sa, sb) {
            if debug {
                eprintln!("RESCUE-SUB [{ssa:.2},{sse:.2}] dur {:.2}", sse - ssa);
            }
            if sse - ssa < MIN_SUB_WINDOW_SECS {
                continue;
            }
            let i0 = (ssa * SAMPLE_RATE as f64) as usize;
            let i1 = ((sse * SAMPLE_RATE as f64) as usize).min(samples.len());
            if i1 <= i0 {
                continue;
            }
            let Some(e) = extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE) else {
                continue;
            };
            let mut sims: Vec<(usize, f32)> =
                used.iter().enumerate().map(|(ci, c)| (ci, cosine(c, &e))).collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            if let (Some(f), Some(s)) = (sims.first(), sims.get(1)) {
                if debug {
                    eprintln!("RESCUE-VOTE [{ssa:.2},{sse:.2}] sp{} margin {:.3}", f.0, f.1 - s.1);
                }
                votes.push(RescueSubWindow {
                    start_secs: ssa,
                    end_secs: sse,
                    cluster: f.0,
                    margin: f.1 - s.1,
                });
            }
        }
        gaps.push(RescueGap {
            gap_start_secs: ga,
            gap_end_secs: gb,
            raw_span: Some((sa, sb)),
            sub_windows: votes,
        });
    }
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_span_in_clips_rows_to_gap() {
        let spans = vec![(5.0, 32.5), (40.0, 45.0)];
        assert_eq!(union_span_in(&spans, 14.0, 16.0), Some((14.0, 16.0)));
        assert_eq!(union_span_in(&spans, 33.0, 39.0), None);
        // hull semantics (spec: bounding span of intersecting rows, clipped)
        assert_eq!(union_span_in(&spans, 31.0, 41.0), Some((31.0, 41.0)));
    }

    #[test]
    fn borrow_cap_stays_in_sync_with_the_render_borrow() {
        // the rescue's modeled winner must track the render's borrow cap or
        // the contradiction gate silently diverges from assign_engine_gap_fragments
        assert_eq!(
            BORROW_CAP_MS,
            crate::audio::speaker::commands::GAP_BORROW_MAX_MS
        );
    }

    #[test]
    fn segment_voiced_sub_windows_finds_and_merges_bursts() {
        let sr = SAMPLE_RATE as usize;
        let mut s = vec![0.01f32; sr * 3]; // quiet room tone (-40 dBFS)
        let burst = |buf: &mut Vec<f32>, at_s: f64, dur_s: f64, amp: f32| {
            let i0 = (at_s * sr as f64) as usize;
            let n = (dur_s * sr as f64) as usize;
            for i in 0..n {
                buf[i0 + i] = amp * ((i as f32) * 0.05).sin();
            }
        };
        burst(&mut s, 1.00, 0.30, 0.30);
        burst(&mut s, 1.45, 0.30, 0.30); // 150ms gap after the first: NOT merged
        let subs = segment_voiced_sub_windows(&s, 0.0, 3.0);
        assert_eq!(subs.len(), 2, "{subs:?}");
        assert!((subs[0].0 - 1.0).abs() < 0.05, "{subs:?}");
        // a 60ms dip inside one burst merges back into it (gap < MERGE_GAP_FRAMES)
        let mut s2 = vec![0.01f32; sr * 3];
        burst(&mut s2, 1.00, 0.20, 0.30);
        burst(&mut s2, 1.26, 0.30, 0.30); // 60ms dip at 1.20-1.26
        let subs2 = segment_voiced_sub_windows(&s2, 0.0, 3.0);
        assert_eq!(subs2.len(), 1, "{subs2:?}");
        // quiet-only audio yields nothing
        let subs3 = segment_voiced_sub_windows(&s, 2.5, 3.0);
        assert!(subs3.is_empty());
    }
}
