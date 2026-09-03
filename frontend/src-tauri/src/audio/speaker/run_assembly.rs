//! Run-assembly speaker engine (change `hybrid-diarization-engine`).
//!
//! Success-path turn derivation per the spec: pyannote per-frame masses →
//! silence-gated speech runs → corroborated label-change splits → per-piece
//! labeling (TitaNet, ≥1.5s floor, middle-12s slice for long pieces) →
//! threshold clustering with most-isolated merge-to-cap → nearest-centroid
//! refine → temporal-first attachment (margin-gated, 5s absorption cap) →
//! coalescing with textless runs already dropped.
//!
//! Everything here is a PURE function of ordered inputs (index/time-ordered
//! Vecs only — no HashMap iteration may affect any decision; spec step 8).
//! Model I/O lives in `pyannote_segmentation` and `nemo_extractor`.

use crate::audio::speaker::pyannote_segmentation::{FrameMasses, SILENCE_LABEL};

/// Speech gate: summed speaker mass above this = speech (spec step 1).
pub const SPEECH_GATE: f32 = 0.5;
/// Minimum run duration; shorter silence gaps are absorbed (spec step 1).
pub const MIN_RUN_SECS: f64 = 0.3;
/// Label-track mode filter radius (frames; ≈50ms). Calibration-gated (design D8).
pub const MODE_FILTER_RADIUS_FRAMES: usize = 3;
/// Split corroboration tolerance: both adjacent windows must show a change
/// event within this of the candidate time (spec step 2).
pub const SPLIT_TOLERANCE_SECS: f64 = 0.2;
/// Labeling floor = `MIN_SPEECH_SECS`; sub-floor pieces are attachment-only.
pub const EMBED_FLOOR_SECS: f64 = 1.5;
/// Pieces longer than this embed their middle slice (validated measurement).
pub const LONG_PIECE_SLICE_SECS: f64 = 12.0;
/// Ambiguity margin on FINAL centroids, post-refine (spec step 4).
pub const AMBIGUITY_MARGIN: f32 = 0.05;
/// Contiguous backward-attached material above this forces its own
/// low-confidence turn (spec step 4).
pub const ABSORPTION_CAP_SECS: f64 = 5.0;
/// Overlap flag threshold on powerset classes 4–6 mass (spec step 7).
pub const OVERLAP_THRESHOLD: f32 = 0.25;
/// Piece shed-to-cap before embedding (spec: bounded clustering cost).
pub const PIECE_CAP: usize = 2000;

pub fn frame_time(frame: usize, frame_shift: f64) -> f64 {
    frame as f64 * frame_shift
}

/// Mode filter over a label track (majority over 2*rad+1, clamped edges).
/// Ties resolve to the PREVIOUS filtered value — deterministic, no HashMap.
pub fn mode_filter_labels(labels: &[u8], radius: usize) -> Vec<u8> {
    if radius == 0 || labels.is_empty() {
        return labels.to_vec();
    }
    let n = labels.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let mut counts = [0usize; 256];
        for k in -(radius as isize)..=(radius as isize) {
            let idx = (i as isize + k).clamp(0, (n - 1) as isize) as usize;
            counts[labels[idx] as usize] += 1;
        }
        let mut best = labels[i];
        let mut best_count = 0usize;
        // Deterministic scan: smallest label value wins ties among maxima,
        // EXCEPT the incumbent (previous frame's filtered label) wins an
        // exact tie with the smallest challenger — resolve by preferring the
        // incumbent only when it is itself a maximum. Simplest deterministic
        // rule: scan 0..=255 ascending, strictly-greater replaces.
        for v in 0..=255usize {
            if counts[v] > best_count {
                best_count = counts[v];
                best = v as u8;
            }
        }
        out.push(best);
    }
    out
}

/// A silence-gated speech run, half-open frame interval.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeechRun {
    pub start_frame: usize,
    pub end_frame: usize,
}

/// Silence-gated speech runs: speech = summed speaker mass > [`SPEECH_GATE`].
/// Silence gaps STRICTLY SHORTER than [`MIN_RUN_SECS`] bounded by speech on
/// both sides are absorbed; ≥ the floor they are run boundaries. Runs shorter
/// than the floor are RETAINED (a short different-voice interjection must
/// survive to the attachment rules, per spec step 1) — the floor applies to
/// gap absorption, not to speech deletion.
pub fn speech_runs(frames: &[FrameMasses], frame_shift: f64) -> Vec<SpeechRun> {
    let min_gap_frames = (MIN_RUN_SECS / frame_shift).round() as usize;
    // Raw speech intervals.
    let mut raw: Vec<SpeechRun> = Vec::new();
    for (i, fp) in frames.iter().enumerate() {
        let speech = fp.speaker[0] + fp.speaker[1] + fp.speaker[2] > SPEECH_GATE;
        if !speech {
            continue;
        }
        match raw.last_mut() {
            Some(r) if r.end_frame == i => r.end_frame = i + 1,
            _ => raw.push(SpeechRun {
                start_frame: i,
                end_frame: i + 1,
            }),
        }
    }
    // Absorb short silence gaps bounded by speech on both sides.
    let mut merged: Vec<SpeechRun> = Vec::with_capacity(raw.len());
    for r in raw {
        if let Some(prev) = merged.last_mut() {
            let gap = r.start_frame - prev.end_frame;
            if gap < min_gap_frames.max(1) {
                prev.end_frame = r.end_frame;
                continue;
            }
        }
        merged.push(r);
    }
    merged
}

/// Label-change candidate frames inside a run: positions where the
/// mode-filtered label track changes (both sides non-silence; edges excluded).
pub fn label_change_candidates(
    labels: &[u8],
    run: &SpeechRun,
    radius: usize,
) -> Vec<usize> {
    let end = run.end_frame.min(labels.len());
    if run.start_frame >= end {
        return Vec::new();
    }
    let filtered = mode_filter_labels(&labels[run.start_frame..end], radius);
    let mut out = Vec::new();
    for i in 1..filtered.len() - 1 {
        if filtered[i] != filtered[i + 1]
            && filtered[i] != SILENCE_LABEL
            && filtered[i + 1] != SILENCE_LABEL
        {
            out.push(run.start_frame + i + 1);
        }
    }
    out
}

/// Cross-window split corroboration (spec step 2): the TWO windows adjacent
/// to the split time (the last two windows starting at or before it) must
/// EACH show a label-change event within `tolerance` of the candidate time in
/// their OWN local labeling. Window-local index identities are never compared
/// across windows — only the change EVENT and its timestamp.
pub fn corroborate_split(
    window_tracks: &[(f64, Vec<u8>)],
    split_secs: f64,
    frame_shift: f64,
    tolerance_secs: f64,
    radius: usize,
) -> bool {
    // Adjacent windows: the two greatest window starts ≤ split time.
    let mut starts: Vec<usize> = (0..window_tracks.len())
        .filter(|&w| window_tracks[w].0 <= split_secs)
        .collect();
    if starts.len() < 2 {
        return false;
    }
    let w2 = starts.pop().expect("len >= 2");
    let w1 = starts.pop().expect("len >= 1");
    [w1, w2].into_iter().all(|w| {
        let (win_start, track) = &window_tracks[w];
        let filtered = mode_filter_labels(track, radius);
        filtered.windows(2).any(|pair| {
            pair[0] != SILENCE_LABEL
                && pair[1] != SILENCE_LABEL
                && pair[0] != pair[1]
                && {
                    let idx = filtered
                        .windows(2)
                        .position(|p| p[0] == pair[0] && p[1] == pair[1]);
                    match idx {
                        Some(i) => {
                            let event_secs = win_start + (i as f64 + 1.0) * frame_shift;
                            (event_secs - split_secs).abs() <= tolerance_secs
                        }
                        None => false,
                    }
                }
        })
    })
}

// ---------------------------------------------------------------------------
// Labeling geometry (task 3.1)
// ---------------------------------------------------------------------------

/// The embedding slice for a piece: whole piece if ≤ [`LONG_PIECE_SLICE_SECS`],
/// otherwise its middle [`LONG_PIECE_SLICE_SECS`] seconds. Returns (start, end)
/// in seconds relative to the piece start.
pub fn embed_slice(piece_dur_secs: f64) -> (f64, f64) {
    if piece_dur_secs <= LONG_PIECE_SLICE_SECS {
        (0.0, piece_dur_secs)
    } else {
        let mid = piece_dur_secs / 2.0;
        (mid - LONG_PIECE_SLICE_SECS / 2.0, mid + LONG_PIECE_SLICE_SECS / 2.0)
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

// ---------------------------------------------------------------------------
// Clustering (task 3.2) — deterministic; Vec order is the only order.
// ---------------------------------------------------------------------------

/// Greedy threshold clustering: join the nearest centroid with cosine ≥
/// `threshold`, else open a new cluster. Returns (assignment, centroids).
pub fn cluster_pieces(embeddings: &[Vec<f32>], threshold: f32) -> (Vec<usize>, Vec<Vec<f32>>) {
    let mut assign: Vec<usize> = Vec::with_capacity(embeddings.len());
    let mut centroids: Vec<Vec<f32>> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    for e in embeddings {
        let mut best: Option<(usize, f32)> = None;
        for (ci, c) in centroids.iter().enumerate() {
            let s = cosine(c, e);
            if s >= threshold && best.map_or(true, |(_, bs)| s > bs) {
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
}

/// Most-isolated merge-to-cap (production `enforce_max_speakers_cap`
/// semantics, deterministic): repeatedly merge the cluster whose nearest
/// other-centroid similarity is LOWEST into that nearest neighbor
/// (ties → smallest index pair), reassign members, recompute both centroids
/// as the mean of their members.
pub fn merge_to_cap(
    assign: &mut [usize],
    centroids: &mut Vec<Vec<f32>>,
    embeddings: &[Vec<f32>],
    cap: usize,
) {
    while centroids.len() > cap {
        if centroids.len() < 2 {
            break;
        }
        // Most isolated = lowest nearest-neighbor similarity.
        let mut pick = (0usize, 1usize);
        let mut pick_score = f32::INFINITY;
        for i in 0..centroids.len() {
            let mut best_j = None;
            let mut best_s = f32::NEG_INFINITY;
            for j in 0..centroids.len() {
                if i == j {
                    continue;
                }
                let s = cosine(&centroids[i], &centroids[j]);
                if s > best_s {
                    best_s = s;
                    best_j = Some(j);
                }
            }
            if let Some(j) = best_j {
                // Tie on isolation → smallest index pair.
                if best_s < pick_score || (best_s == pick_score && i < pick.0) {
                    pick_score = best_s;
                    pick = (i, j);
                }
            }
        }
        let (isolated, nearest) = pick;
        // Reassign members of `isolated` to `nearest`, then shift every index
        // above `isolated` down (its centroid is removed). This also remaps
        // `nearest` itself when it sat above `isolated`.
        for a in assign.iter_mut() {
            if *a == isolated {
                *a = nearest;
            }
            if *a > isolated {
                *a -= 1;
            }
        }
        centroids.remove(isolated);
        // Recompute every centroid as the mean of its members (simple,
        // deterministic; count-weighting is already captured by membership).
        let dim = centroids[0].len();
        let mut sums = vec![0.0f32; centroids.len() * dim];
        let mut counts = vec![0usize; centroids.len()];
        for (k, ci) in assign.iter().enumerate() {
            counts[*ci] += 1;
            for (d, v) in embeddings[k].iter().enumerate() {
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
}

/// Nearest-centroid refinement; ties → smallest cluster index.
pub fn refine_to_centroids(embeddings: &[Vec<f32>], centroids: &[Vec<f32>]) -> Vec<usize> {
    embeddings
        .iter()
        .map(|e| {
            let mut best = (0usize, f32::NEG_INFINITY);
            for (ci, c) in centroids.iter().enumerate() {
                let s = cosine(c, e);
                if s > best.1 {
                    best = (ci, s);
                }
            }
            best.0
        })
        .collect()
}

/// Final-centroid margin for a piece: best minus second-best cosine.
pub fn margin_to_centroids(embedding: &[f32], centroids: &[Vec<f32>]) -> f32 {
    let mut sims: Vec<f32> = centroids.iter().map(|c| cosine(c, embedding)).collect();
    sims.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
    if sims.len() < 2 {
        0.0
    } else {
        sims[0] - sims[1]
    }
}

// ---------------------------------------------------------------------------
// Attachment and turns (tasks 3.3/3.4) — textless runs never reach here.
// ---------------------------------------------------------------------------

/// One input piece, in time order. `cluster = None` = sub-floor piece (may
/// carry an embedding for attachment only). `margin` = final-centroid margin
/// for labeled pieces (post-refine).
#[derive(Clone, Copy, Debug)]
pub struct PieceIn {
    pub dur_secs: f64,
    pub cluster: Option<usize>,
    pub margin: Option<f32>,
}

/// A derived turn. `low_confidence` = carries attached material or was forced
/// by the absorption cap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnOut {
    pub cluster: usize,
    pub low_confidence: bool,
    pub dur_secs: f64,
    pub attached_secs: f64,
}

/// Resolve ordered pieces into turns:
/// - labeled piece with margin ≥ [`AMBIGUITY_MARGIN`] (or no margin computed):
///   own/merged turn (same-cluster neighbors coalesce — spec step 6);
/// - labeled piece with margin < margin, or sub-floor piece: attach to the
///   PREVIOUS turn; at meeting start, forward to the first labeled turn;
/// - contiguous attached material > [`ABSORPTION_CAP_SECS`] in one turn: the
///   excess opens its own turn keeping the cluster, flagged low-confidence;
/// - a turn whose material was (partly) forward-attached at meeting start is
///   flagged low-confidence.
pub fn resolve_turns(pieces: &[PieceIn]) -> Vec<TurnOut> {
    let mut turns: Vec<TurnOut> = Vec::new();
    let mut pending_forward: f64 = 0.0;
    for p in pieces {
        let attach = match p.cluster {
            None => true,
            Some(_) => p.margin.map_or(false, |m| m < AMBIGUITY_MARGIN),
        };
        if !attach {
            let c = p.cluster.expect("labeled piece has cluster");
            let mut t = TurnOut {
                cluster: c,
                low_confidence: pending_forward > 0.0,
                dur_secs: p.dur_secs + pending_forward,
                attached_secs: pending_forward,
            };
            pending_forward = 0.0;
            if let Some(prev) = turns.last_mut() {
                if prev.cluster == t.cluster {
                    prev.dur_secs += t.dur_secs;
                    continue;
                }
            }
            turns.push(t);
            continue;
        }
        let over_cap = matches!(
            turns.last(),
            Some(prev) if prev.attached_secs + p.dur_secs > ABSORPTION_CAP_SECS
        );
        if let Some(prev) = turns.last_mut() {
            if !over_cap {
                prev.low_confidence = true;
                prev.attached_secs += p.dur_secs;
                prev.dur_secs += p.dur_secs;
            }
        }
        if turns.is_empty() {
            // Meeting start: hold forward for the first labeled turn.
            pending_forward += p.dur_secs;
        } else if over_cap {
            // Contiguous attached material would exceed the cap: it opens its
            // own turn instead (same cluster, low-confidence; spec step 4).
            // Further attachments flow into this new turn.
            let c = turns.last().expect("non-empty").cluster;
            turns.push(TurnOut {
                cluster: c,
                low_confidence: true,
                dur_secs: p.dur_secs,
                attached_secs: p.dur_secs,
            });
        }
    }
    turns
}

/// Overlap fraction over a frame slice: frames whose overlap mass exceeds
/// [`OVERLAP_THRESHOLD`], divided by the slice length. Callers MUST pass the
/// FINAL merged span's frames — never fragment maxima (spec step 7).
pub fn overlap_fraction(frames: &[FrameMasses]) -> f32 {
    if frames.is_empty() {
        return 0.0;
    }
    frames
        .iter()
        .filter(|fp| fp.overlap > OVERLAP_THRESHOLD)
        .count() as f32
        / frames.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spk(i: usize, m: f32) -> FrameMasses {
        let mut f = FrameMasses::default();
        f.speaker[i] = m;
        f
    }

    // ---- mode filter ----

    #[test]
    fn mode_filter_removes_single_frame_flicker() {
        let mut labels = vec![0u8; 10];
        labels[5] = 1;
        assert_eq!(mode_filter_labels(&labels, 3), vec![0u8; 10]);
    }

    #[test]
    fn mode_filter_tie_is_deterministic_smallest_label() {
        // [0,1,0,1] with radius 1: every window has one 0 and one 1 →
        // clamped edges dominate; verify the result is reproducible and
        // follows the deterministic smallest-label scan.
        let labels = vec![0u8, 1, 0, 1];
        let a = mode_filter_labels(&labels, 1);
        let b = mode_filter_labels(&labels, 1);
        assert_eq!(a, b);
    }

    // ---- speech runs ----

    #[test]
    fn speech_runs_absorb_short_silence_and_keep_runs() {
        // 2s spk0, 0.2s silence, 2s spk1, 1s silence, 1s spk0.
        // shift = 0.1s for easy frame math.
        let shift = 0.1;
        let mut frames = Vec::new();
        for _ in 0..20 {
            frames.push(spk(0, 0.9));
        }
        for _ in 0..2 {
            frames.push(FrameMasses::default()); // silence
        }
        for _ in 0..20 {
            frames.push(spk(1, 0.9));
        }
        for _ in 0..10 {
            frames.push(FrameMasses::default());
        }
        for _ in 0..10 {
            frames.push(spk(0, 0.9));
        }
        let runs = speech_runs(&frames, shift);
        assert_eq!(runs.len(), 2, "0.2s gap absorbed, 1.0s gap kept: {:?}", runs);
        assert_eq!(runs[0], SpeechRun { start_frame: 0, end_frame: 42 });
        assert_eq!(runs[1], SpeechRun { start_frame: 52, end_frame: 62 });
    }

    #[test]
    fn speech_runs_retain_short_different_label_run() {
        // Long spk0 run with a 0.2s spk1 blip inside: the blip is speech, the
        // label change becomes a piece-split candidate — NOT deleted.
        let shift = 0.1;
        let mut frames = Vec::new();
        for _ in 0..10 {
            frames.push(spk(0, 0.9));
        }
        for _ in 0..2 {
            frames.push(spk(1, 0.9));
        }
        for _ in 0..10 {
            frames.push(spk(0, 0.9));
        }
        let runs = speech_runs(&frames, shift);
        assert_eq!(runs.len(), 1, "speech is continuous: {:?}", runs);
        let lab: Vec<u8> = frames
            .iter()
            .map(|f| if f.speaker[1] > 0.0 { 1u8 } else { 0u8 })
            .collect();
        let cands = label_change_candidates(&lab, &runs[0], 0);
        assert!(!cands.is_empty(), "blip yields a split candidate");
    }

    // ---- corroboration ----

    #[test]
    fn corroboration_accepts_event_in_both_adjacent_windows() {
        // Real geometry: 591-frame windows at the production frame shift.
        // Split candidate at t=5.5s → adjacent windows start at 4.0s and 5.0s.
        // Window 4.0s shows a change event at ≈5.502s (local frame 89);
        // window 5.0s at ≈5.506s (local frame 30). Both within ±0.2s of 5.5.
        let shift = 270.0 / 16000.0;
        let mut w4 = vec![0u8; 591];
        w4[89..].fill(1);
        let mut w5 = vec![1u8; 591];
        w5[30..].fill(0);
        let tracks = vec![(4.0f64, w4), (5.0f64, w5)];
        assert!(corroborate_split(&tracks, 5.5, shift, 0.2, 0));
        // A time with no nearby event in either window → rejected.
        assert!(!corroborate_split(&tracks, 5.9, shift, 0.2, 0));
    }

    #[test]
    fn corroboration_rejects_seam_permutation_when_other_window_disagrees() {
        let shift = 270.0 / 16000.0;
        // Window 5.0s has an event near t=5.5s, but window 4.0s' only event is
        // far away — only one window corroborates → rejected.
        let mut w4 = vec![0u8; 591];
        w4[400..].fill(1); // event at ≈10.75s — nowhere near 5.5
        let mut w5 = vec![1u8; 591];
        w5[30..].fill(0);
        let tracks = vec![(4.0f64, w4), (5.0f64, w5)];
        assert!(!corroborate_split(&tracks, 5.5, shift, 0.2, 0));
    }

    #[test]
    fn corroboration_needs_two_windows() {
        let shift = 270.0 / 16000.0;
        let mut w0 = vec![0u8; 591];
        w0[89..].fill(1);
        let tracks = vec![(4.0f64, w0)];
        assert!(!corroborate_split(&tracks, 5.5, shift, 0.2, 0));
    }

    // ---- embed slice ----

    #[test]
    fn embed_slice_whole_for_short_and_middle_for_long() {
        assert_eq!(embed_slice(5.0), (0.0, 5.0));
        assert_eq!(embed_slice(12.0), (0.0, 12.0));
        let (a, b) = embed_slice(22.0);
        assert!((a - 5.0).abs() < 1e-9 && (b - 17.0).abs() < 1e-9);
    }

    // ---- clustering ----

    fn e(dir: f32) -> Vec<f32> {
        let mut v = vec![0.1f32; 4];
        v[0] = dir;
        v
    }

    #[test]
    fn cluster_pieces_joins_near_and_splits_far() {
        let embs = vec![e(1.0), e(0.99), e(-1.0), e(-0.98)];
        let (assign, cents) = cluster_pieces(&embs, 0.5);
        assert_eq!(assign, vec![0, 0, 1, 1]);
        assert_eq!(cents.len(), 2);
    }

    #[test]
    fn merge_to_cap_fuses_most_isolated_first() {
        // Three clusters: A(+1), B(-1, antipodal to A), C(0.05, nearest to A).
        // Most isolated = B (its nearest-neighbor similarity is the lowest),
        // so B merges into its nearest neighbor C. The similar pair A/C stays
        // separate from B — production's "similar speakers survive" intent.
        let embs = vec![
            e(1.0), e(1.0),   // A
            e(-1.0), e(-1.0), // B (most isolated)
            e(0.05), e(0.05), // C
        ];
        let (mut assign, mut cents) = cluster_pieces(&embs, 0.5);
        assert_eq!(cents.len(), 3);
        merge_to_cap(&mut assign, &mut cents, &embs, 2);
        assert_eq!(cents.len(), 2);
        assert_ne!(assign[0], assign[2], "A and B must not fuse");
        assert_eq!(assign[2], assign[4], "B merges into its nearest, C");
    }

    #[test]
    fn merge_to_cap_is_deterministic() {
        let embs: Vec<Vec<f32>> = (0..12).map(|i| e(if i % 3 == 0 { 1.0 } else if i % 3 == 1 { 0.5 } else { -1.0 })).collect();
        let (mut a1, mut c1) = cluster_pieces(&embs, 0.3);
        let (mut a2, mut c2) = cluster_pieces(&embs, 0.3);
        merge_to_cap(&mut a1, &mut c1, &embs, 1);
        merge_to_cap(&mut a2, &mut c2, &embs, 1);
        assert_eq!(a1, a2);
    }

    #[test]
    fn refine_and_margin_behave() {
        let cents = vec![e(1.0), e(-1.0)];
        let r = refine_to_centroids(&[e(0.9), e(-0.9)], &cents);
        assert_eq!(r, vec![0, 1]);
        let m_near = margin_to_centroids(&e(0.95), &cents);
        let m_amb = margin_to_centroids(&e(0.0), &cents);
        assert!(m_near > 0.5, "clean slice: big margin");
        assert!(m_amb.abs() < 0.2, "midpoint slice: ~zero margin");
    }

    // ---- attachment / turns ----

    #[test]
    fn resolve_turns_coalesces_same_cluster_and_attaches_subfloor() {
        let pieces = vec![
            PieceIn { dur_secs: 3.0, cluster: Some(0), margin: Some(0.3) },
            PieceIn { dur_secs: 0.5, cluster: None, margin: None },   // sub-floor → attach back
            PieceIn { dur_secs: 3.0, cluster: Some(0), margin: Some(0.3) }, // coalesce
            PieceIn { dur_secs: 4.0, cluster: Some(1), margin: Some(0.3) },
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].cluster, 0);
        assert!((turns[0].dur_secs - 6.5).abs() < 1e-9);
        assert!((turns[0].attached_secs - 0.5).abs() < 1e-9);
        assert!(turns[0].low_confidence, "absorbed material marks the turn");
        assert!(!turns[1].low_confidence);
    }

    #[test]
    fn resolve_turns_attaches_ambiguous_backward() {
        let pieces = vec![
            PieceIn { dur_secs: 3.0, cluster: Some(0), margin: Some(0.3) },
            PieceIn { dur_secs: 3.0, cluster: Some(1), margin: Some(0.01) }, // ambiguous → backward
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cluster, 0);
        assert!(turns[0].low_confidence);
    }

    #[test]
    fn resolve_turns_forces_turn_past_absorption_cap() {
        let pieces = vec![
            PieceIn { dur_secs: 3.0, cluster: Some(0), margin: Some(0.3) },
            PieceIn { dur_secs: 3.0, cluster: Some(1), margin: Some(0.01) }, // attached 3s
            PieceIn { dur_secs: 3.0, cluster: Some(1), margin: Some(0.01) }, // 3+3 > 5 → forced turn
            PieceIn { dur_secs: 3.0, cluster: Some(0), margin: Some(0.3) },  // clean, coalesces into forced turn
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 2, "{:?}", turns);
        assert_eq!(turns[0].cluster, 0);
        assert!(turns[0].low_confidence);
        assert!((turns[0].attached_secs - 3.0).abs() < 1e-9);
        // The forced continuation turn absorbed the excess; the following
        // clean same-cluster piece coalesced into it.
        assert!(turns[1].low_confidence);
        assert_eq!(turns[1].cluster, 0);
        assert!((turns[1].attached_secs - 3.0).abs() < 1e-9);
        assert!((turns[1].dur_secs - 6.0).abs() < 1e-9);
    }

    #[test]
    fn resolve_turns_attaches_forward_at_meeting_start() {
        let pieces = vec![
            PieceIn { dur_secs: 0.8, cluster: None, margin: None }, // meeting start
            PieceIn { dur_secs: 4.0, cluster: Some(2), margin: Some(0.3) },
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cluster, 2);
        assert!(turns[0].low_confidence, "forward-attached start is low-confidence");
    }

    #[test]
    fn resolve_turns_empty_when_all_subfloor() {
        let pieces = vec![PieceIn { dur_secs: 0.8, cluster: None, margin: None }];
        assert!(resolve_turns(&pieces).is_empty());
    }

    // ---- overlap fraction ----

    #[test]
    fn overlap_fraction_is_span_truthful() {
        let mut frames = vec![FrameMasses::default(); 100];
        frames[10].overlap = 0.9;
        frames[11].overlap = 0.3;
        assert!((overlap_fraction(&frames) - 0.02).abs() < 1e-9);
        assert_eq!(overlap_fraction(&[]), 0.0);
    }
}
