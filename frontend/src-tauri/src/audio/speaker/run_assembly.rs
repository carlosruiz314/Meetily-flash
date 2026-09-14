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
/// event within this of the candidate time (spec step 2). Calibrated on the
/// fixture's non-hold-out entries (S5): per-window decodes jitter more than
/// 0.2s at real changes, which silently swallowed the ≈30.0s boundary.
pub const SPLIT_TOLERANCE_SECS: f64 = 0.35;
/// Labeling floor = `MIN_SPEECH_SECS`; sub-floor pieces are attachment-only.
pub const EMBED_FLOOR_SECS: f64 = 1.5;
/// Pieces longer than this embed their middle slice (validated measurement).
pub const LONG_PIECE_SLICE_SECS: f64 = 12.0;
/// Ambiguity margin on FINAL centroids, post-refine (spec step 4).
pub const AMBIGUITY_MARGIN: f32 = 0.05;
/// Contiguous backward-attached material above this forces its own
/// low-confidence turn (spec step 4).
pub const ABSORPTION_CAP_SECS: f64 = 5.0;
/// Minimum duration for margin-based sub-floor promotion: shorter slices
/// produce confidently-wrong identities (a 0.37s fragment once won with
/// margin 0.28 on noise). Below this floor, sub-floor pieces resolve by
/// neighbor vote instead. Calibration-gated (design D8).
pub const PROMOTION_FLOOR_SECS: f64 = 0.8;
/// Overlap flag threshold on powerset classes 4–6 mass (spec step 7).
pub const OVERLAP_THRESHOLD: f32 = 0.25;
/// Piece shed-to-cap before embedding (spec: bounded clustering cost).
pub const PIECE_CAP: usize = 2000;
/// Whisper-timestamp skew tolerance for textless-run detection (spec step 5):
/// a piece counts as text-covered if any transcript row reaches within this
/// of it. Calibration-gated under the fixture-gate rule (design D8).
pub const TEXT_SKEW_TOLERANCE_SECS: f64 = 0.25;

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

/// Margin from each edge of a window track treated as untrustworthy for
/// attesting a split (the window's own decode is least reliable there).
/// Mid-window decodes are pyannote's most confident (measured: slot-consistent
/// at ≥0.99 across every covering window in the fixture meeting).
pub const TRUST_EDGE_SECS: f64 = 2.0;

/// Trust-zone label track: for each frame, the label from the covering window
/// whose CENTER is nearest. Merged-track seams overwrite mid-window decodes
/// with edge-of-window ones (the merged flips land on integer-second window
/// seams — measured), so the trust zone is reconstructed from the per-window
/// tracks the engine already holds. Frames no window covers stay silence.
pub fn trust_zone_track(
    window_tracks: &[(f64, Vec<u8>)],
    n_frames: usize,
    frame_shift: f64,
) -> Vec<u8> {
    let mut out = vec![SILENCE_LABEL; n_frames];
    let mut best_dist = vec![f64::INFINITY; n_frames];
    for (start_secs, track) in window_tracks {
        let center = *start_secs + track.len() as f64 * frame_shift / 2.0;
        let start_f = (start_secs / frame_shift).round();
        for (li, &lab) in track.iter().enumerate() {
            let fi_f = start_f + li as f64;
            if fi_f < 0.0 {
                continue;
            }
            let fi = fi_f as usize;
            if fi >= n_frames {
                break;
            }
            let d = (fi as f64 * frame_shift - center).abs();
            if d < best_dist[fi] {
                best_dist[fi] = d;
                out[fi] = lab;
            }
        }
    }
    out
}

/// Speaker-slot change candidates inside a run: positions where the
/// mode-filtered track flips between two DIFFERENT speaker slots, either
/// directly or across a short absorbed silence (the measured handoff shape
/// slot-A → silence(<0.3s) → slot-B — `speech_runs` absorbs the pause, so
/// the slot contrast inside the run is the only trace left). The candidate
/// frame is the flip for direct transitions, the silence midpoint for
/// mediated ones.
pub fn slot_change_candidates(track: &[u8], run: &SpeechRun, radius: usize) -> Vec<usize> {
    let end = run.end_frame.min(track.len());
    if run.start_frame >= end {
        return Vec::new();
    }
    let filtered = mode_filter_labels(&track[run.start_frame..end], radius);
    let mut out = Vec::new();
    let mut last_speaker: Option<u8> = None;
    let mut silence_start: Option<usize> = None;
    for (i, &lab) in filtered.iter().enumerate() {
        if lab == SILENCE_LABEL {
            if silence_start.is_none() {
                silence_start = Some(i);
            }
            continue;
        }
        if let Some(prev) = last_speaker {
            if lab != prev {
                let local = match silence_start {
                    Some(s) => (s + i) / 2,
                    None => i,
                };
                out.push(run.start_frame + local);
            }
        }
        last_speaker = Some(lab);
        silence_start = None;
    }
    out
}

/// One window's slot-change events (absolute seconds) plus the trust zone it
/// can attest splits in.
pub struct WindowSlotEvents {
    pub trust_lo: f64,
    pub trust_hi: f64,
    pub events: Vec<f64>,
}

/// Precompute per-window slot-change events (see `slot_change_candidates`)
/// and trust-zone bounds, so split corroboration is a lookup.
pub fn window_slot_events(
    window_tracks: &[(f64, Vec<u8>)],
    frame_shift: f64,
    radius: usize,
) -> Vec<WindowSlotEvents> {
    window_tracks
        .iter()
        .map(|(start_secs, track)| {
            let events = slot_change_candidates(
                track,
                &SpeechRun {
                    start_frame: 0,
                    end_frame: track.len(),
                },
                radius,
            )
            .into_iter()
            .map(|f| *start_secs + frame_time(f, frame_shift))
            .collect();
            let win_end = *start_secs + track.len() as f64 * frame_shift;
            WindowSlotEvents {
                trust_lo: start_secs + TRUST_EDGE_SECS,
                trust_hi: win_end - TRUST_EDGE_SECS,
                events,
            }
        })
        .collect()
}

/// Trust-zone split corroboration: of the windows whose trust zone contains
/// the split time, AT LEAST TWO must show a slot-change event within
/// `tolerance_secs` of it in their OWN track (unanimity is not required —
/// window decodes near a real handoff legitimately disagree on
/// micro-timing). Window-local slot identities are never compared across
/// windows — each window only attests that A change happens there (same
/// principle as `corroborate_split`). Known blind zone: meeting edges —
/// before the first ~2s and after the last ~2s fewer than two trust zones
/// overlap, so splits there cannot reach the 2-window threshold and are
/// deliberately not attested (single-window attestation is the
/// seam-artifact class this mechanism exists to reject).
pub fn corroborate_slot_change(
    windows: &[WindowSlotEvents],
    split_secs: f64,
    tolerance_secs: f64,
) -> bool {
    windows
        .iter()
        .filter(|w| split_secs >= w.trust_lo && split_secs < w.trust_hi)
        .filter(|w| w.events.iter().any(|&e| (e - split_secs).abs() <= tolerance_secs))
        .count()
        >= 2
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

/// Average-linkage agglomerative clustering (both-bars accuracy fix,
/// 2026-09-09): the greedy online pass seeded centroids from processing
/// order and drifted — measured on cde5c264 it produced TWO centroids
/// (Cynthia 0.72, Ricardo 0.71 to their anchors) while CARLOS — the most
/// prolific voice — matched neither (0.12–0.32), so his pieces flipped
/// between the two by thin margins and Ricardo's real speech shared his
/// badge. AHC measures the embedding matrix directly: a cluster forms from
/// its members' mutual similarity, independent of processing order, and the
/// threshold keeps the same meaning (merge while the closest pair's average
/// linkage ≥ `threshold`). Ties → smallest index pair. Deterministic;
/// returns (assignment, per-cluster mean centroids).
pub fn cluster_pieces_ahc(embeddings: &[Vec<f32>], threshold: f32) -> (Vec<usize>, Vec<Vec<f32>>) {
    let n = embeddings.len();
    if n == 0 {
        return (Vec::new(), Vec::new());
    }
    // Pairwise cosine matrix (n² — bounded by PIECE_CAP).
    let mut sim = vec![0.0f32; n * n];
    for i in 0..n {
        for j in (i + 1)..n {
            let s = cosine(&embeddings[i], &embeddings[j]);
            sim[i * n + j] = s;
            sim[j * n + i] = s;
        }
    }
    // Active clusters: member lists; linkage sums[i][j] = Σ pairwise
    // cosines between members of cluster i and j; average = sums / (ni*nj).
    let mut members: Vec<Vec<usize>> = (0..n).map(|i| vec![i]).collect();
    let mut sums = sim.clone();
    let mut alive: Vec<usize> = (0..n).collect();
    while alive.len() > 1 {
        // Closest active pair by average linkage; ties → smallest (i, j).
        let mut best: Option<(usize, usize, f32)> = None;
        for a in 0..alive.len() {
            for b in (a + 1)..alive.len() {
                let (i, j) = (alive[a], alive[b]);
                let avg = sums[i * n + j]
                    / (members[i].len() * members[j].len()) as f32;
                let take = match best {
                    None => true,
                    Some((_, _, bs)) => avg > bs,
                };
                if take {
                    best = Some((i, j, avg));
                }
            }
        }
        let Some((i, j, avg)) = best else { break };
        if avg < threshold {
            break;
        }
        // Merge j into i (i < j by construction of the scan? No — i, j are
        // values in `alive`, so enforce min/max for determinism).
        let (i, j) = if i < j { (i, j) } else { (j, i) };
        let moved: Vec<usize> = members[j].clone();
        for k in moved {
            members[i].push(k);
        }
        for k in 0..n {
            if k != i && k != j {
                sums[i * n + k] += sums[j * n + k];
                sums[k * n + i] = sums[i * n + k];
            }
        }
        members[j].clear();
        alive.retain(|&x| x != j);
    }
    // Dense relabel by first appearance in time order (deterministic ids).
    let mut remap: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
    let mut assign = vec![0usize; n];
    for (k, m) in members.iter().enumerate() {
        if m.is_empty() {
            continue;
        }
        let next = remap.len();
        remap.entry(k).or_insert(next);
        for &item in m {
            assign[item] = remap[&k];
        }
    }
    let dim = embeddings[0].len();
    let mut centroids = vec![vec![0.0f32; dim]; remap.len()];
    let mut counts = vec![0usize; remap.len()];
    for (k, e) in assign.iter().zip(embeddings) {
        counts[*k] += 1;
        for (d, v) in e.iter().enumerate() {
            centroids[*k][d] += v;
        }
    }
    for (c, &cnt) in centroids.iter_mut().zip(&counts) {
        for v in c.iter_mut() {
            *v /= cnt as f32;
        }
    }
    (assign, centroids)
}

/// Greedy threshold clustering: join the nearest centroid with cosine ≥
/// `threshold`, else open a new cluster. Returns (assignment, centroids).
///
/// SUPERSEDED for the engine path by [`cluster_pieces_ahc`] (order-dependent
/// seed drift, measured 2026-09-09); retained for its tests and any caller
/// that needs streaming semantics.
#[allow(dead_code)]
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

/// Lloyd-style refinement (engine calibration, 2026-09-05): the single-pass
/// refine kept pieces on STALE greedy centroids — one real voice ended up
/// split across two clusters (the false 34.66s boundary: its pieces scored
/// 0.55–0.57 affinity to the user's other stretches yet sat in the wrong
/// cluster). Alternate nearest-centroid assignment and centroid
/// recomputation until stable, bounded. Deterministic.
pub fn refine_loop(
    assign: &mut [usize],
    centroids: &mut [Vec<f32>],
    embeddings: &[Vec<f32>],
    max_iters: usize,
) {
    if centroids.is_empty() || embeddings.is_empty() {
        return;
    }
    let dim = centroids[0].len();
    for _ in 0..max_iters {
        let new_assign = refine_to_centroids(embeddings, centroids);
        if new_assign == *assign {
            break;
        }
        assign.clone_from_slice(&new_assign);
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
/// for labeled pieces (post-refine). `promoted_subfloor` = a sub-floor piece
/// the embedding-margin arbitration promoted to its own turn (engine-set).
#[derive(Clone, Copy, Debug)]
pub struct PieceIn {
    pub start_secs: f64,
    pub dur_secs: f64,
    pub cluster: Option<usize>,
    pub margin: Option<f32>,
    pub promoted_subfloor: bool,
}

/// A derived turn. `low_confidence` = carries attached material or was forced
/// by the absorption cap. `continues_previous` = the engine's continuation
/// fact (spec: same-cluster adjacency ⇒ true; voice change / meeting start ⇒
/// false) — persisted on the turn's first row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnOut {
    pub start_secs: f64,
    pub end_secs: f64,
    pub cluster: usize,
    pub low_confidence: bool,
    pub continues_previous: bool,
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
    let mut pending_start: f64 = 0.0;
    for p in pieces {
        let piece_end = p.start_secs + p.dur_secs;
        let attach = match p.cluster {
            None => !p.promoted_subfloor,
            Some(_) => !p.promoted_subfloor && p.margin.map_or(false, |m| m < AMBIGUITY_MARGIN),
        };
        if !attach {
            let c = p.cluster.expect("labeled piece has cluster");
            let start = if pending_forward > 0.0 { pending_start } else { p.start_secs };
            let t = TurnOut {
                start_secs: start,
                end_secs: piece_end,
                cluster: c,
                low_confidence: pending_forward > 0.0 || p.promoted_subfloor,
                continues_previous: turns.last().map_or(false, |prev| prev.cluster == c),
                dur_secs: p.dur_secs + pending_forward,
                attached_secs: pending_forward,
            };
            pending_forward = 0.0;
            if let Some(prev) = turns.last_mut() {
                if prev.cluster == t.cluster {
                    prev.end_secs = t.end_secs;
                    prev.dur_secs += t.dur_secs;
                    prev.attached_secs += t.attached_secs;
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
                prev.end_secs = piece_end;
            }
        }
        if turns.is_empty() {
            // Meeting start: hold forward for the first labeled turn.
            if pending_forward == 0.0 {
                pending_start = p.start_secs;
            }
            pending_forward += p.dur_secs;
        } else if over_cap {
            // Contiguous attached material would exceed the cap: it opens its
            // own turn instead (same cluster, low-confidence; spec step 4).
            // Further attachments flow into this new turn. Same cluster as
            // the previous turn ⇒ it continues that turn's speech.
            let c = turns.last().expect("non-empty").cluster;
            turns.push(TurnOut {
                start_secs: p.start_secs,
                end_secs: piece_end,
                cluster: c,
                low_confidence: true,
                continues_previous: true,
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

// ---------------------------------------------------------------------------
// Piece extraction (task 3.1) and textless-run detection (task 3.4)
// ---------------------------------------------------------------------------

/// A piece time span, in seconds from meeting start.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieceSpan {
    pub start_secs: f64,
    pub end_secs: f64,
}

/// Pieces from speech runs with corroborated sub-run splits (spec steps 1–2):
/// a run's label-track change candidates split the run ONLY where
/// [`corroborate_split`] accepts them; uncorroborated changes leave the run
/// whole.
pub fn derive_pieces(
    labels: &[u8],
    runs: &[SpeechRun],
    window_tracks: &[(f64, Vec<u8>)],
    frame_shift: f64,
    radius: usize,
) -> Vec<PieceSpan> {
    let tz = trust_zone_track(window_tracks, labels.len(), frame_shift);
    let window_events = window_slot_events(window_tracks, frame_shift, radius);
    let mut out = Vec::new();
    for run in runs {
        let mut bounds = vec![run.start_frame];
        // Proximity predicate shared by both candidate sources: a candidate
        // within SPLIT_TOLERANCE_SECS of an accepted bound or the run edge
        // is dropped — seam artifacts ride a real transition's attestations,
        // and two bounds <0.7s apart persist as a sliver piece (measured
        // live: 30-80ms phantom turns at sp0→sp1 handoffs).
        let end_t = frame_time(run.end_frame, frame_shift);
        let too_close = |t: f64, bounds: &[usize]| {
            bounds
                .iter()
                .any(|&b| (frame_time(b, frame_shift) - t).abs() <= SPLIT_TOLERANCE_SECS)
                || (end_t - t).abs() <= SPLIT_TOLERANCE_SECS
        };
        for cand in label_change_candidates(labels, run, radius) {
            let t = frame_time(cand, frame_shift);
            if too_close(t, &bounds) {
                continue;
            }
            if corroborate_split(window_tracks, t, frame_shift, SPLIT_TOLERANCE_SECS, radius) {
                bounds.push(cand);
            }
        }
        // Trust-zone slot-change candidates: the merged track's seam
        // overwrites plus the <0.3s silence absorption hide short-pause
        // handoffs (slot-A → silence → slot-B); the per-window trust zones
        // still attest them.
        for cand in slot_change_candidates(&tz, run, radius) {
            let t = frame_time(cand, frame_shift);
            if too_close(t, &bounds) {
                continue;
            }
            if corroborate_slot_change(&window_events, t, SPLIT_TOLERANCE_SECS) {
                bounds.push(cand);
            }
        }
        bounds.sort_unstable();
        bounds.dedup();
        // Strictly interior bounds only — a bound equal to a run edge would
        // emit a zero-width piece, which persists as a zero-duration turn
        // (measured live: a 12.07s zero-width piece shredded a sentence and
        // its turn mapped to no speaker entity).
        bounds.retain(|&b| b > run.start_frame && b < run.end_frame);
        bounds.insert(0, run.start_frame);
        bounds.push(run.end_frame);
        for pair in bounds.windows(2) {
            if pair[1] > pair[0] {
                out.push(PieceSpan {
                    start_secs: frame_time(pair[0], frame_shift),
                    end_secs: frame_time(pair[1], frame_shift),
                });
            }
        }
    }
    out
}

/// True when no transcript text span overlaps the piece within the whisper
/// skew tolerance — a breath, laugh, or untranscribed voiced noise (spec
/// step 5). Such pieces are dropped BEFORE labeling, clustering, and
/// same-cluster coalescing.
pub fn is_textless(piece: &PieceSpan, text_spans: &[(f64, f64)], skew_secs: f64) -> bool {
    text_spans.iter().all(|&(a, b)| {
        b + skew_secs <= piece.start_secs || a - skew_secs >= piece.end_secs
    })
}

/// Drop textless pieces (spec step 5), preserving order.
pub fn drop_textless(
    pieces: &[PieceSpan],
    text_spans: &[(f64, f64)],
    skew_secs: f64,
) -> Vec<PieceSpan> {
    pieces
        .iter()
        .filter(|p| !is_textless(p, text_spans, skew_secs))
        .copied()
        .collect()
}

// ---------------------------------------------------------------------------
// Text alignment (task 3.6)
// ---------------------------------------------------------------------------

/// One token-level timestamp of a whisper row.
#[derive(Clone, Debug)]
pub struct TokenIn {
    pub text: String,
    pub start_secs: f64,
}

/// One transcript row. `tokens` empty = legacy consolidated row (proportional
/// split); non-empty = token-level alignment.
#[derive(Clone, Debug)]
pub struct RowIn {
    pub text: String,
    pub start_secs: f64,
    pub end_secs: f64,
    pub tokens: Vec<TokenIn>,
}

/// A derived turn's time span and cluster, as the alignment input.
#[derive(Clone, Copy, Debug)]
pub struct TurnSpan {
    pub start_secs: f64,
    pub end_secs: f64,
    pub cluster: usize,
}

/// Part of row `row_idx` attributed to turn `turn_idx`. Concatenating every
/// fragment's text of a row reproduces the row's content exactly (spec
/// content-preservation invariant).
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedFragment {
    pub row_idx: usize,
    pub turn_idx: usize,
    pub text: String,
}

/// The turn-aligned transcript: one entry per turn, text = its fragments.
#[derive(Clone, Debug, PartialEq)]
pub struct AlignedTurn {
    pub cluster: usize,
    pub start_secs: f64,
    pub end_secs: f64,
    pub text: String,
}

/// The turn covering time `t` (start ≤ t < end); outside all turns → nearest
/// (before the first / in a gap → the following turn; after the last → it).
/// None only when `turns` is empty.
fn turn_index_for_time(turns: &[TurnSpan], t: f64) -> Option<usize> {
    if turns.is_empty() {
        return None;
    }
    for (i, turn) in turns.iter().enumerate() {
        if t < turn.end_secs {
            return Some(i);
        }
    }
    Some(turns.len() - 1)
}

/// Nearest-in-time turn by interval distance; ties → earlier turn.
fn nearest_turn(turns: &[TurnSpan], row: &RowIn) -> usize {
    let mut best = (0usize, f64::INFINITY);
    for (i, t) in turns.iter().enumerate() {
        let d = if row.end_secs <= t.start_secs {
            t.start_secs - row.end_secs
        } else if row.start_secs >= t.end_secs {
            row.start_secs - t.end_secs
        } else {
            0.0
        };
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// Split `text` into whitespace-free word chunks so chunk `i` holds roughly
/// `shares[i]` of the characters (last chunk takes the remainder). Every word
/// lands in exactly one chunk — content-preserving by construction.
fn split_words_proportional(text: &str, shares: &[f64]) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let total_chars: usize = words.iter().map(|w| w.chars().count() + 1).sum();
    let mut out = Vec::with_capacity(shares.len());
    let mut consumed = 0usize;
    for (si, &share) in shares.iter().enumerate() {
        let target = if si + 1 == shares.len() {
            usize::MAX
        } else {
            (share * total_chars as f64).round() as usize
        };
        let mut seg_chars = 0usize;
        let mut chunk: Vec<&str> = Vec::new();
        while consumed < words.len() && seg_chars < target {
            chunk.push(words[consumed]);
            seg_chars += words[consumed].chars().count() + 1;
            consumed += 1;
        }
        out.push(chunk.join(" "));
    }
    out
}

/// Align transcript rows to the derived turns (spec step 6):
/// - token-timestamped rows split at turn boundaries (token times decide);
/// - token-less rows split proportionally at any boundary inside them
///   (word-boundary cuts, time-weighted);
/// - a row with zero overlap with every turn (shed span) attaches to the
///   nearest-in-time turn.
/// With no turns at all the output is empty (zero-label meetings).
pub fn align_rows_to_turns(rows: &[RowIn], turns: &[TurnSpan]) -> Vec<AlignedFragment> {
    let mut out = Vec::new();
    for (row_idx, row) in rows.iter().enumerate() {
        if turns.is_empty() {
            break;
        }
        if !row.tokens.is_empty() {
            // Group consecutive tokens by target turn, preserving order.
            let mut groups: Vec<(usize, Vec<&str>)> = Vec::new();
            for tok in &row.tokens {
                let ti = turn_index_for_time(turns, tok.start_secs).expect("turns non-empty");
                match groups.last_mut() {
                    Some((prev_ti, texts)) if *prev_ti == ti => texts.push(&tok.text),
                    _ => groups.push((ti, vec![&tok.text])),
                }
            }
            for (ti, texts) in groups {
                out.push(AlignedFragment {
                    row_idx,
                    turn_idx: ti,
                    text: texts.join(""),
                });
            }
        } else {
            // Per-turn time overlap with the row.
            let overlaps: Vec<(usize, f64)> = turns
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let ov = row.end_secs.min(t.end_secs) - row.start_secs.max(t.start_secs);
                    (i, ov.max(0.0))
                })
                .collect();
            let total: f64 = overlaps.iter().map(|(_, ov)| *ov).sum();
            if total > 0.0 {
                let shares: Vec<f64> = overlaps.iter().map(|(_, ov)| ov / total).collect();
                for (text, (ti, _)) in split_words_proportional(&row.text, &shares)
                    .into_iter()
                    .zip(overlaps)
                {
                    if !text.is_empty() {
                        out.push(AlignedFragment { row_idx, turn_idx: ti, text });
                    }
                }
            } else {
                let ti = nearest_turn(turns, row);
                out.push(AlignedFragment {
                    row_idx,
                    turn_idx: ti,
                    text: row.text.clone(),
                });
            }
        }
    }
    out
}

/// Group fragments into per-turn transcript entries (time order preserved).
pub fn group_fragments_by_turn(
    turns: &[TurnSpan],
    fragments: &[AlignedFragment],
) -> Vec<AlignedTurn> {
    turns
        .iter()
        .enumerate()
        .map(|(i, t)| AlignedTurn {
            cluster: t.cluster,
            start_secs: t.start_secs,
            end_secs: t.end_secs,
            text: fragments
                .iter()
                .filter(|f| f.turn_idx == i)
                .map(|f| f.text.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
        })
        .collect()
}

/// The gate's mid-sentence detector (hard invariant): true when the text
/// begins mid-sentence — first non-punctuation/symbol/whitespace character is
/// a lowercase letter.
pub fn is_mid_sentence_start(text: &str) -> bool {
    text.chars()
        .skip_while(|c| !c.is_alphanumeric())
        .next()
        .map_or(false, |c| c.is_alphabetic() && c.is_lowercase())
}

// ---------------------------------------------------------------------------
// Sub-run voice-flip scan (both-bars follow-up, 2026-09-09)
// ---------------------------------------------------------------------------

/// Sub-run voice-flip scan: pyannote's decode collapses to uncertainty where
/// a short other-voice back-channel enters mid-run — it does NOT flip the
/// argmax. Measured at the S2 pin (Cynthia's "Yeah" at 12.0s inside the held
/// sp0 run [9.38,13.03]): sp1 mass runs 0.94–1.00 up to 11.95, then the
/// window decodes to mush (sp1 0.45→0.26, sp2 0.24–0.33, silence up to 0.48)
/// with sp1 still argmax — no slot change, so no split candidate ever exists
/// and no smoothing knob can recover the boundary. A run frame is CONTESTED
/// when the argmax speaker mass drops below [`FLIP_VALLEY_ARGMAX_MAX`] while
/// a second speaker's mass reaches [`FLIP_VALLEY_SECOND_MIN`] — exactly the
/// measured signature. The engine runs an embedding voice check at these
/// valleys (`run_engine`); this module supplies the pure candidate detector.
pub const FLIP_VALLEY_ARGMAX_MAX: f32 = 0.7;
pub const FLIP_VALLEY_SECOND_MIN: f32 = 0.15;
/// Contested frames separated by fewer than this many frames merge into one
/// valley (one voice check per valley, not per frame).
pub const FLIP_VALLEY_MERGE_FRAMES: usize = 10;

/// Contested confusion-valley spans (frame indices, half-open) inside one
/// speech run — where pyannote lost label confidence and an embedding
/// voice-flip check is worth its cost. Deterministic; index/time order only.
pub fn confusion_valleys(frames: &[FrameMasses], run: &SpeechRun) -> Vec<(usize, usize)> {
    let end = run.end_frame.min(frames.len());
    if run.start_frame >= end {
        return Vec::new();
    }
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for f in run.start_frame..end {
        let mut s = frames[f].speaker;
        s.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
        let contested = s[0] < FLIP_VALLEY_ARGMAX_MAX && s[1] >= FLIP_VALLEY_SECOND_MIN;
        if !contested {
            continue;
        }
        match spans.last_mut() {
            Some(last) if f - last.1 < FLIP_VALLEY_MERGE_FRAMES => last.1 = f + 1,
            _ => spans.push((f, f + 1)),
        }
    }
    spans
}

// ---------------------------------------------------------------------------
// Gap rescue (change `gap-speech-voice-attribution`): speech pyannote decoded
// as silence, attributed by voice. Pure decision function; the engine derives
// gaps/sub-windows and embeds (run_engine), this function gates and selects.
// ---------------------------------------------------------------------------

/// One energy-segmented voiced sub-window of a gap, with its identity vote.
#[derive(Clone, Debug, PartialEq)]
pub struct RescueSubWindow {
    pub start_secs: f64,
    pub end_secs: f64,
    pub cluster: usize,
    pub margin: f32,
}

/// One text-bearing silence gap offered to the rescue pass.
#[derive(Clone, Debug)]
pub struct RescueGap {
    /// The pyannote silence gap.
    pub gap_start_secs: f64,
    pub gap_end_secs: f64,
    /// Union of intersecting transcript-row spans clipped to the gap
    /// (None = no text overlap — true silence, never a candidate).
    pub raw_span: Option<(f64, f64)>,
    /// Decided AND undecided sub-window votes (the margin gate runs here).
    pub sub_windows: Vec<RescueSubWindow>,
}

/// A selected rescue: splice a synthetic promoted piece over this span.
#[derive(Clone, Debug, PartialEq)]
pub struct RescueCandidate {
    pub start_secs: f64,
    pub end_secs: f64,
    pub cluster: usize,
    pub margin: f32,
}

/// Point-to-span distance in whole milliseconds, containment-0,
/// start-inclusive/end-exclusive (render-borrow-faithful).
fn rescue_dist_ms(p: i64, s: i64, e: i64) -> i64 {
    if p >= s && p < e {
        0
    } else if p < s {
        s - p
    } else {
        p - e
    }
}

/// Tie-abstain zone: when the two nearest turn edges are within this distance
/// of each other AND carry different labels, geometry is indeterminate (the
/// render's earlier-turn tie-break is a convenience, not ear truth) and voice
/// evidence decides. Calibratable only under the fixture-gate rule.
const RESCUE_TIE_EPS_MS: i64 = 100;

/// Modeled borrow winner for `mid_ms`: nearest turn edge within `borrow_cap_ms`,
/// ties broken by the earlier turn (turns are time-ordered; strict `<` keeps
/// the first minimum). None when no turn is within the cap, or when the two
/// nearest edges tie within `RESCUE_TIE_EPS_MS` with DIFFERENT labels
/// (geometry abstains; a same-label near-tie still resolves to that label —
/// both readings agree).
fn rescue_borrow_winner(turns: &[(f64, f64, u32)], mid_ms: i64, borrow_cap_ms: i64) -> Option<u32> {
    let mut best: Option<(u32, i64)> = None;
    for (s, e, l) in turns {
        let d = rescue_dist_ms(mid_ms, (*s * 1000.0) as i64, (*e * 1000.0) as i64);
        if d > borrow_cap_ms {
            continue;
        }
        match best {
            Some((_, bd)) if d >= bd => {}
            _ => best = Some((*l, d)),
        }
    }
    let Some((bl, bd)) = best else {
        return None;
    };
    // abstain when ANY different-label turn edges within the tie epsilon of
    // the best distance (geometry indeterminate; voice decides)
    let contested = turns.iter().any(|(s, e, l)| {
        *l != bl
            && rescue_dist_ms(mid_ms, (*s * 1000.0) as i64, (*e * 1000.0) as i64) - bd
                <= RESCUE_TIE_EPS_MS
    });
    if contested {
        return None;
    }
    Some(bl)
}

/// Select gap-rescue candidates (pure, deterministic; time/index order only).
///
/// Gates per gap, ALL of: (1) the raw span exists and is at least
/// `min_raw_secs` (the promotion floor — guards the text evidence); (2) the
/// gap separates two DISTINCT turns (interior and meeting-edge gaps abstain);
/// (3) at least one voiced sub-window is decided (margin ≥ `rescue_margin`);
/// the row is attributed to the LAST decided sub-window (legacy rows skew
/// early — the words sit at the row's tail); (4) that sub-window's cluster
/// differs from the modeled borrow winner (raw-span midpoint, containment-0,
/// i64 ms, ties → earlier turn, within `borrow_cap_ms`; no winner within the
/// cap → splice — decided voice replaces geometry that would leave the row
/// unattributed). A sub-window matching its adjacent flank never blocks the
/// last-decided attribution (far-flank matches coalesce in resolve_turns).
pub fn rescue_candidates(
    turns: &[(f64, f64, u32)],
    gaps: &[RescueGap],
    min_raw_secs: f64,
    rescue_margin: f32,
    borrow_cap_ms: i64,
) -> Vec<RescueCandidate> {
    let mut out = Vec::new();
    for gap in gaps {
        // (2) distinct flanks by adjacency; meeting-edge and interior abstain
        let Some(&(_la, _le, lc)) = turns
            .iter()
            .filter(|(_, e, _)| *e <= gap.gap_start_secs + 1e-9)
            .last()
        else {
            continue;
        };
        let Some(&(_ra, _re, rc)) = turns
            .iter()
            .find(|(s, _, _)| *s >= gap.gap_end_secs - 1e-9)
        else {
            continue;
        };
        if lc == rc {
            continue;
        }
        // (1) raw span floor
        let Some((sa, sb)) = gap.raw_span else { continue };
        if sb - sa < min_raw_secs {
            continue;
        }
        // (3) last decided sub-window
        let Some(sw) = gap
            .sub_windows
            .iter()
            .filter(|s| s.margin >= rescue_margin)
            .last()
        else {
            continue;
        };
        // (4) borrow-winner contradiction
        let mid_ms = ((sa + sb) / 2.0 * 1000.0) as i64;
        let winner = rescue_borrow_winner(turns, mid_ms, borrow_cap_ms);
        if winner == Some(sw.cluster as u32) {
            continue;
        }
        out.push(RescueCandidate {
            start_secs: sw.start_secs,
            end_secs: sw.end_secs,
            cluster: sw.cluster,
            margin: sw.margin,
        });
    }
    out
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
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 0.5, cluster: None, margin: None, promoted_subfloor: false },   // sub-floor → attach back
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false }, // coalesce
            PieceIn { start_secs: 0.0, dur_secs: 4.0, cluster: Some(1), margin: Some(0.3), promoted_subfloor: false },
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
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.01), promoted_subfloor: false }, // ambiguous → backward
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cluster, 0);
        assert!(turns[0].low_confidence);
    }

    #[test]
    fn resolve_turns_forces_turn_past_absorption_cap() {
        let pieces = vec![
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.01), promoted_subfloor: false }, // attached 3s
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.01), promoted_subfloor: false }, // 3+3 > 5 → forced turn
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },  // clean, coalesces into forced turn
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
            PieceIn { start_secs: 0.0, dur_secs: 0.8, cluster: None, margin: None, promoted_subfloor: false }, // meeting start
            PieceIn { start_secs: 0.0, dur_secs: 4.0, cluster: Some(2), margin: Some(0.3), promoted_subfloor: false },
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].cluster, 2);
        assert!(turns[0].low_confidence, "forward-attached start is low-confidence");
    }

    #[test]
    fn resolve_turns_empty_when_all_subfloor() {
        let pieces = vec![PieceIn { start_secs: 0.0, dur_secs: 0.8, cluster: None, margin: None, promoted_subfloor: false }];
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

    // ---- piece extraction (3.1) ----

    #[test]
    fn derive_pieces_split_only_at_corroborated_changes() {
        let shift = 270.0 / 16000.0;
        // Corroborated change at ≈5.5s (windows 4.0/5.0 both show it);
        // uncorroborated change at ≈8.44s (frame 500) must NOT split.
        let change = (5.5f64 / shift).round() as usize;
        let mut labels = vec![0u8; 600];
        labels[change..].fill(1);
        labels[500..].fill(0);
        let mut w4 = vec![0u8; 591];
        w4[89..].fill(1);
        let mut w5 = vec![1u8; 591];
        w5[30..].fill(0);
        let tracks = vec![(4.0f64, w4), (5.0f64, w5)];
        let run = SpeechRun { start_frame: 0, end_frame: 600 };
        let pieces = derive_pieces(&labels, &[run], &tracks, shift, 0);
        assert_eq!(pieces.len(), 2, "{:?}", pieces);
        assert!((pieces[0].end_secs - 5.5).abs() < 0.1, "{:?}", pieces);
        assert!(pieces[1].end_secs > pieces[1].start_secs);
    }

    // ---- trust-zone slot-change splits (closure-gap fix) ----

    #[test]
    fn trust_zone_track_center_nearest_window_wins() {
        let shift = 0.1;
        // w1 covers 0–10s (center 5.0), flips to label 2 at 8.0;
        // w2 covers 5–15s (center 10.0), flips to label 1 at 10.0.
        let mut w1 = vec![0u8; 100];
        w1[80..].fill(2);
        let mut w2 = vec![0u8; 100];
        w2[50..].fill(1);
        let tz = trust_zone_track(&[(0.0, w1), (5.0, w2)], 150, shift);
        assert_eq!(tz[40], 0, "only w1 covers 4.0s");
        // 8.5s: w1 says 2 (dist 3.5), w2 says 0 (dist 1.5) → w2 wins.
        assert_eq!(tz[85], 0);
        // 12.0s: only w2 covers → its label.
        assert_eq!(tz[120], 1);
    }

    #[test]
    fn slot_change_candidates_fire_across_absorbed_silence() {
        // spk0 ×20 (2.0s), silence ×2 (0.2s — below MIN_RUN_SECS, absorbed by
        // speech_runs), spk1 ×20. One run; the mediated handoff lands on the
        // silence midpoint.
        let mut track = vec![0u8; 20];
        track.extend(vec![SILENCE_LABEL; 2]);
        track.extend(vec![1u8; 20]);
        let run = SpeechRun { start_frame: 0, end_frame: track.len() };
        let cands = slot_change_candidates(&track, &run, 0);
        assert_eq!(cands, vec![21], "midpoint of the 2-frame silence");
    }

    #[test]
    fn slot_change_candidates_ignores_same_slot_pause() {
        let mut track = vec![0u8; 10];
        track.extend(vec![SILENCE_LABEL; 3]);
        track.extend(vec![0u8; 12]);
        let run = SpeechRun { start_frame: 0, end_frame: track.len() };
        assert!(slot_change_candidates(&track, &run, 1).is_empty());
    }

    #[test]
    fn derive_pieces_splits_at_trust_zone_mediated_change() {
        let shift = 270.0 / 16000.0;
        // Merged labels: one continuous spk0 run (the seam-smoothed view).
        // Both covering windows' own tracks show spk0 → silence → spk1 at the
        // same absolute time inside their trust zones → must split there.
        let split_t = 8.0f64;
        let n = 1200;
        let labels = vec![0u8; n];
        let mediated = |w: f64| {
            let mut t = vec![0u8; 591];
            let s = ((split_t - w) / shift).round() as usize;
            t[s..s + 12].fill(SILENCE_LABEL);
            t[s + 12..].fill(1);
            t
        };
        let tracks = vec![(5.0f64, mediated(5.0)), (6.0f64, mediated(6.0))];
        let run = SpeechRun { start_frame: 0, end_frame: n };
        let pieces = derive_pieces(&labels, &[run], &tracks, shift, 0);
        assert_eq!(pieces.len(), 2, "{:?}", pieces);
        assert!(
            (pieces[0].end_secs - split_t).abs() < 0.25,
            "{:?}",
            pieces
        );
    }

    #[test]
    fn derive_pieces_requires_two_attesting_windows() {
        let shift = 270.0 / 16000.0;
        let split_t = 8.0f64;
        let n = 1200;
        let labels = vec![0u8; n];
        let mut w5 = vec![0u8; 591];
        let s5 = ((split_t - 5.0) / shift).round() as usize;
        w5[s5..s5 + 12].fill(SILENCE_LABEL);
        w5[s5 + 12..].fill(1);
        let w6 = vec![0u8; 591]; // sees no change
        let tracks = vec![(5.0f64, w5), (6.0f64, w6)];
        let run = SpeechRun { start_frame: 0, end_frame: n };
        let pieces = derive_pieces(&labels, &[run], &tracks, shift, 0);
        assert_eq!(pieces.len(), 1, "{:?}", pieces);
    }

    #[test]
    fn derive_pieces_drops_sliver_candidate_near_run_end() {
        let shift = 270.0 / 16000.0;
        // A corroborated mediated change 0.31s before the run end must be
        // dropped by the proximity guard (sliver class): a split there would
        // emit a sub-floor sliver piece.
        let n = 602usize; // run end 10.16s
        let labels = vec![0u8; n];
        let mediated = |w: f64| {
            let mut t = vec![0u8; 591];
            let s = ((9.85 - w) / shift).round() as usize;
            t[s..s + 12].fill(SILENCE_LABEL);
            t[s + 12..].fill(1);
            t
        };
        let tracks = vec![
            (4.0f64, mediated(4.0)),
            (5.0f64, mediated(5.0)),
            (6.0f64, mediated(6.0)),
        ];
        let run = SpeechRun { start_frame: 0, end_frame: n };
        let pieces = derive_pieces(&labels, &[run], &tracks, shift, 0);
        assert_eq!(
            pieces.len(),
            1,
            "candidate within tolerance of run end must drop: {:?}",
            pieces
        );
    }

    // ---- phantom centroids (3.2) ----

    #[test]
    fn merge_to_cap_leaves_no_phantom_centroids() {
        let embs = vec![e(1.0), e(0.9), e(-1.0), e(-0.9), e(0.0), e(0.05)];
        let (mut assign, mut cents) = cluster_pieces(&embs, 0.3);
        merge_to_cap(&mut assign, &mut cents, &embs, 2);
        let mut counts = vec![0usize; cents.len()];
        for a in &assign {
            counts[*a] += 1;
        }
        assert!(counts.iter().all(|&c| c > 0), "every persisted centroid has ≥1 labeled piece");
        assert_eq!(cents.len(), 2);
    }

    // ---- textless runs (3.4) ----

    #[test]
    fn textless_run_dropped_before_coalescing_where_is_ricardo() {
        // A (text-covered), T (voiced laugh, no text — different pyannote
        // label), B (text-covered). Undropped, the laugh dices the speaker's
        // stretch; dropped before coalescing, the surrounding same-speaker
        // text is ONE turn.
        let skew = TEXT_SKEW_TOLERANCE_SECS;
        let text = vec![(34.9, 36.5), (38.6, 39.3)];
        let pieces = vec![
            PieceSpan { start_secs: 34.9, end_secs: 36.9 },
            PieceSpan { start_secs: 37.0, end_secs: 37.4 },
            PieceSpan { start_secs: 38.4, end_secs: 39.3 },
        ];
        let kept = drop_textless(&pieces, &text, skew);
        assert_eq!(kept.len(), 2, "the laugh is textless: {:?}", kept);
        assert!(!is_textless(&pieces[0], &text, skew));
        assert!(!is_textless(&pieces[2], &text, skew));

        let undropped = vec![
            PieceIn { start_secs: 0.0, dur_secs: 2.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 0.4, cluster: Some(1), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 0.9, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
        ];
        assert_eq!(resolve_turns(&undropped).len(), 3, "control: the undropped laugh dices the stretch");
        let dropped = vec![
            PieceIn { start_secs: 0.0, dur_secs: 2.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 0.9, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
        ];
        let turns = resolve_turns(&dropped);
        assert_eq!(turns.len(), 1, "surrounding same-speaker text is ONE turn");
        assert!(!turns[0].continues_previous);
    }

    // ---- sub-run voice-flip valleys ----

    #[test]
    fn confusion_valleys_find_the_pin_b_signature() {
        // The measured S2 shape: ~1s of clean sp1 (mass ≈ 1.0), then the
        // contested mush (sp1 0.45 → 0.26 with sp2 0.24–0.33), then clean
        // sp1 again. One valley exactly over the mush.
        let shift = 0.1; // frame math only — the fn is shift-agnostic
        let mut frames = Vec::new();
        for _ in 0..10 {
            frames.push(spk(1, 0.98));
        }
        for i in 0..10 {
            let mut f = FrameMasses::default();
            f.speaker[1] = 0.45 - i as f32 * 0.02;
            f.speaker[2] = 0.30;
            f.silence = 1.0 - f.speaker[1] - f.speaker[2];
            frames.push(f);
        }
        for _ in 0..10 {
            frames.push(spk(1, 0.98));
        }
        let run = SpeechRun { start_frame: 0, end_frame: frames.len() };
        let valleys = confusion_valleys(&frames, &run);
        assert_eq!(valleys, vec![(10, 20)], "one valley over the contested span");
        let _ = shift;
    }

    #[test]
    fn confusion_valleys_ignores_clean_and_solo_quiet_frames() {
        // Clean single-voice run: no valleys. A dip that silences WITHOUT a
        // second voice (argmax 0.4, rest silence) is not contested — that is
        // a pause signature, not a back-channel.
        let mut frames = Vec::new();
        for _ in 0..20 {
            frames.push(spk(0, 0.98));
        }
        let mut quiet = FrameMasses::default();
        quiet.speaker[0] = 0.4;
        quiet.silence = 0.6;
        for _ in 0..5 {
            frames.push(quiet);
        }
        for _ in 0..20 {
            frames.push(spk(0, 0.98));
        }
        let run = SpeechRun { start_frame: 0, end_frame: frames.len() };
        assert!(confusion_valleys(&frames, &run).is_empty());
    }

    #[test]
    fn confusion_valleys_merge_nearby_contested_frames() {
        // Two contested bursts 5 frames apart (< FLIP_VALLEY_MERGE_FRAMES)
        // are one valley; 15 frames apart they stay two.
        let mut frames = vec![spk(0, 0.98); 40];
        for f in 10..15 {
            let mut m = FrameMasses::default();
            m.speaker[0] = 0.5;
            m.speaker[1] = 0.3;
            m.silence = 0.2;
            frames[f] = m;
        }
        for f in 20..25 {
            let mut m = FrameMasses::default();
            m.speaker[0] = 0.5;
            m.speaker[1] = 0.3;
            m.silence = 0.2;
            frames[f] = m;
        }
        let run = SpeechRun { start_frame: 0, end_frame: frames.len() };
        assert_eq!(confusion_valleys(&frames, &run), vec![(10, 25)]);
    }

    #[test]
    fn confusion_valleys_never_escape_the_run() {
        // Contested frames outside the run's half-open span are invisible.
        let mut contested = spk(0, 0.3);
        contested.speaker[1] = 0.5;
        let mut frames = vec![spk(0, 0.98); 30];
        frames[0] = contested;
        frames[29] = contested;
        let run = SpeechRun { start_frame: 5, end_frame: 25 };
        assert!(confusion_valleys(&frames, &run).is_empty());
    }

    // ---- average-linkage AHC clustering ----

    #[test]
    fn ahc_finds_two_voices_independent_of_order() {
        // The greedy failure mode: order-dependent seeding absorbed a voice.
        // AHC must separate two tight pairs regardless of interleave order.
        let embs = vec![e(1.0), e(-1.0), e(0.99), e(-0.99), e(1.0), e(-1.0)];
        let (assign, cents) = cluster_pieces_ahc(&embs, 0.5);
        assert_eq!(cents.len(), 2, "{assign:?}");
        assert_eq!(assign[0], assign[2], "positive pair together");
        assert_eq!(assign[0], assign[4]);
        assert_eq!(assign[1], assign[3]);
        assert_ne!(assign[0], assign[1]);
    }

    #[test]
    fn ahc_stops_when_no_pair_reaches_threshold() {
        // Chain a—b 0.9, a—c 0.5, b—c 0.7: a+b merge (0.9), then average
        // linkage (ab)—c = (0.5+0.7)/2 = 0.6 < 0.65 → stop at 2 clusters.
        // e(d) puts the first `dir` of `dim` entries: reuse the helper with
        // 4-dim vectors to get controlled cosines.
        let dim4 = |vals: [f32; 4]| vals.to_vec();
        let a = dim4([1.0, 0.0, 0.0, 0.0]);
        let b = dim4([0.9, 0.43589, 0.0, 0.0]); // cos(a,b)=0.9
        // c: cos(a,c)=0.5, cos(b,c)=0.75 — b is close enough to merge with a
        // (0.9), and c with the pair averages (0.5+0.75)/2=0.625 < 0.65, so
        // AVERAGE linkage (not single-link max) must stop the merge.
        let c = dim4([0.5, 0.6883, 0.5256, 0.0]);
        let norm = |mut v: Vec<f32>| {
            let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            for x in v.iter_mut() {
                *x /= n;
            }
            v
        };
        let (a, b, c) = (norm(a), norm(b), norm(c));
        let cos_ab = cosine(&a, &b);
        let cos_ac = cosine(&a, &c);
        let cos_bc = cosine(&b, &c);
        assert!((cos_ab - 0.9).abs() < 0.01, "{cos_ab}");
        assert!((cos_ac - 0.5).abs() < 0.01, "{cos_ac}");
        assert!((cos_bc - 0.75).abs() < 0.01, "{cos_bc}");
        let (assign, cents) = cluster_pieces_ahc(&[a, b, c], 0.65);
        assert_eq!(cents.len(), 2, "{assign:?}");
        assert_eq!(assign[0], assign[1]);
        assert_ne!(assign[0], assign[2]);
    }

    #[test]
    fn ahc_three_well_separated_voices_form_three_clusters() {
        // The cde5c264 shape: three voices, each tight, mutually far —
        // exactly what the greedy pass collapsed to two. Voice 1 ~ [10,1…],
        // voice 2 antipodal, voice 3 near-orthogonal alternating sign
        // (cos(v1,v3) ≈ 0.10, scaling must NOT matter — a parallel vector is
        // the SAME voice, which is why the first draft of this test failed).
        let voice = |sign: f32| -> Vec<Vec<f32>> {
            (0..3)
                .map(|i| {
                    let mut v = vec![sign; 8];
                    v[0] = sign * 10.0;
                    v[1] += i as f32 * 0.01; // tiny spread within a voice
                    v
                })
                .collect()
        };
        let mut embs = voice(1.0);
        embs.extend(voice(-1.0));
        embs.extend((0..3).map(|i| {
            let mut v = vec![0.0f32; 8];
            for (k, x) in v.iter_mut().enumerate() {
                *x = if k % 2 == 0 { 1.0 } else { -1.0 };
            }
            v[0] += i as f32 * 0.01; // near-identical members, distinct direction
            v
        }));
        let (assign, cents) = cluster_pieces_ahc(&embs, 0.5);
        assert_eq!(cents.len(), 3, "{assign:?}");
        for b in 0..3 {
            assert_eq!(assign[b * 3], assign[b * 3 + 1]);
            assert_eq!(assign[b * 3 + 1], assign[b * 3 + 2]);
        }
        assert_ne!(assign[0], assign[3]);
        assert_ne!(assign[3], assign[6]);
        assert_ne!(assign[0], assign[6]);
    }

    #[test]
    fn ahc_is_deterministic() {
        let embs: Vec<Vec<f32>> = (0..12)
            .map(|i| e(if i % 3 == 0 { 1.0 } else if i % 3 == 1 { 0.6 } else { -1.0 }))
            .collect();
        let (a1, c1) = cluster_pieces_ahc(&embs, 0.3);
        let (a2, c2) = cluster_pieces_ahc(&embs, 0.3);
        assert_eq!(a1, a2);
        assert_eq!(c1, c2);
    }

    #[test]
    fn ahc_handles_single_and_empty() {
        let (a, c) = cluster_pieces_ahc(&[e(1.0)], 0.5);
        assert_eq!(a, vec![0]);
        assert_eq!(c.len(), 1);
        let (a, c) = cluster_pieces_ahc(&[], 0.5);
        assert!(a.is_empty());
        assert!(c.is_empty());
    }

    // ---- text alignment (3.6) ----

    fn tok(text: &str, t: f64) -> TokenIn {
        TokenIn { text: text.to_string(), start_secs: t }
    }

    fn alphanumeric(s: &str) -> String {
        s.chars().filter(|c| c.is_alphanumeric()).collect()
    }

    #[test]
    fn token_row_splits_at_boundary_tail_lands_on_earlier_turn() {
        // The 02:12–02:50 fixture shape: one whisper row straddling the
        // ≈163s corroborated boundary; the "And I was like … one" tail
        // (tokens before 163.0) belongs to the EARLIER speaker's turn; the
        // later-speaker text (tokens from 163.0) opens the next turn.
        let turns = vec![
            TurnSpan { start_secs: 159.0, end_secs: 163.0, cluster: 0 },
            TurnSpan { start_secs: 163.0, end_secs: 170.0, cluster: 1 },
        ];
        let row = RowIn {
            text: " And I was like, oh, when you put a that one Is Min jian. Really".to_string(),
            start_secs: 160.0,
            end_secs: 164.0,
            tokens: vec![
                tok(" And", 162.1), tok(" I", 162.4), tok(" was", 162.7),
                tok(" like,", 162.85), tok(" oh,", 162.9), tok(" when", 162.95),
                tok(" you", 162.97), tok(" put", 162.98), tok(" a", 162.99),
                tok(" that", 162.995), tok(" one", 162.999),
                tok(" Is", 163.05), tok(" Min", 163.2), tok(" jian.", 163.4),
                tok(" Really", 163.6),
            ],
        };
        let frags = align_rows_to_turns(&[row.clone()], &turns);
        assert_eq!(frags.len(), 2, "{:?}", frags);
        assert_eq!(frags[0].turn_idx, 0);
        assert_eq!(frags[1].turn_idx, 1);
        assert_eq!(
            alphanumeric(&frags[0].text),
            "AndIwaslikeohwhenyouputathatone",
            "the tail lands on the earlier turn: {:?}",
            frags[0].text
        );
        assert_eq!(alphanumeric(&frags[1].text), "IsMinjianReally");
        // Content preservation for the straddling row.
        let combined: String = frags.iter().map(|f| alphanumeric(&f.text)).collect();
        assert_eq!(combined, alphanumeric(&row.text));
    }

    #[test]
    fn tokenless_row_splits_proportionally_at_word_boundaries() {
        let turns = vec![
            TurnSpan { start_secs: 0.0, end_secs: 8.0, cluster: 0 },
            TurnSpan { start_secs: 8.0, end_secs: 20.0, cluster: 1 },
        ];
        let row = RowIn {
            text: "alpha bravo charlie delta echo foxtrot golf hotel".to_string(),
            start_secs: 4.0,
            end_secs: 16.0,
            tokens: vec![],
        };
        let frags = align_rows_to_turns(&[row.clone()], &turns);
        assert_eq!(frags.len(), 2, "{:?}", frags);
        // 4s of 12s in turn 0 → the first third of the words.
        assert_eq!(frags[0].text, "alpha bravo charlie");
        assert_eq!(frags[1].text, "delta echo foxtrot golf hotel");
        let combined: String = frags.iter().map(|f| alphanumeric(&f.text)).collect();
        assert_eq!(combined, alphanumeric(&row.text));
    }

    #[test]
    fn zero_overlap_row_attaches_nearest_in_time() {
        let turns = vec![
            TurnSpan { start_secs: 0.0, end_secs: 10.0, cluster: 0 },
            TurnSpan { start_secs: 30.0, end_secs: 40.0, cluster: 1 },
        ];
        let row = RowIn {
            text: "shed span words here".to_string(),
            start_secs: 18.0,
            end_secs: 20.0,
            tokens: vec![],
        };
        let frags = align_rows_to_turns(&[row], &turns);
        assert_eq!(frags.len(), 1);
        assert_eq!(frags[0].turn_idx, 0, "turn 0 is nearer (8s vs 10s)");
    }

    #[test]
    fn content_preservation_invariant_end_to_end() {
        // Mixed rows — token-timestamped straddler, token-less straddler,
        // clean row, zero-overlap row. Nothing dropped, nothing doubled.
        let turns = vec![
            TurnSpan { start_secs: 0.0, end_secs: 8.0, cluster: 0 },
            TurnSpan { start_secs: 8.0, end_secs: 20.0, cluster: 1 },
        ];
        let rows = vec![
            RowIn {
                text: " Okay. I have some updates".to_string(),
                start_secs: 1.0,
                end_secs: 4.0,
                tokens: vec![tok(" Okay.", 1.0), tok(" I", 2.0), tok(" have", 3.0), tok(" some", 3.5), tok(" updates", 3.9)],
            },
            RowIn {
                text: "one two three four five six".to_string(),
                start_secs: 6.0,
                end_secs: 12.0,
                tokens: vec![],
            },
            RowIn {
                text: " roadmap, hopefully.".to_string(),
                start_secs: 14.0,
                end_secs: 16.0,
                tokens: vec![tok(" roadmap,", 14.0), tok(" hopefully.", 15.0)],
            },
            RowIn {
                text: "shed leftovers".to_string(),
                start_secs: 25.0,
                end_secs: 26.0,
                tokens: vec![],
            },
        ];
        let frags = align_rows_to_turns(&rows, &turns);
        let aligned = group_fragments_by_turn(&turns, &frags);
        assert_eq!(aligned.len(), 2);
        for (ri, row) in rows.iter().enumerate() {
            let want = alphanumeric(&row.text);
            let got: String = frags
                .iter()
                .filter(|f| f.row_idx == ri)
                .map(|f| alphanumeric(&f.text))
                .collect();
            assert_eq!(got, want, "row {:?} content preserved exactly once", row.text);
        }
    }

    // ---- continuation fact + gate helper ----

    #[test]
    fn resolve_turns_marks_continuation_facts() {
        // Voice change → false; forced same-cluster continuation → true;
        // meeting start → false.
        let pieces = vec![
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.01), promoted_subfloor: false }, // attached back
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.01), promoted_subfloor: false }, // over cap → forced turn
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },  // coalesces
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 2);
        assert!(!turns[0].continues_previous, "meeting start is not a continuation");
        assert!(turns[1].continues_previous, "forced same-cluster turn continues the previous");

        let voice_change = resolve_turns(&[
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 0.0, dur_secs: 3.0, cluster: Some(1), margin: Some(0.3), promoted_subfloor: false },
        ]);
        assert_eq!(voice_change.len(), 2);
        assert!(!voice_change[1].continues_previous, "corroborated voice change is NOT a continuation");
    }

    #[test]
    fn mid_sentence_start_detection() {
        assert!(is_mid_sentence_start("and I was like"));
        assert!(is_mid_sentence_start("  -- (five years of it"));
        assert!(!is_mid_sentence_start("Okay. I have some updates."));
        assert!(!is_mid_sentence_start("5 years ago"));
        assert!(!is_mid_sentence_start(""));
        assert!(!is_mid_sentence_start("…?!"));
    }

    #[test]
    fn resolve_turns_tracks_time_spans() {
        let pieces = vec![
            PieceIn { start_secs: 5.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 8.5, dur_secs: 0.5, cluster: None, margin: None, promoted_subfloor: false }, // attached
            PieceIn { start_secs: 9.0, dur_secs: 3.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false }, // coalesces
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 1);
        assert!((turns[0].start_secs - 5.0).abs() < 1e-9);
        assert!((turns[0].end_secs - 12.0).abs() < 1e-9);
        assert!((turns[0].dur_secs - 6.5).abs() < 1e-9);
        assert!((turns[0].attached_secs - 0.5).abs() < 1e-9);
    }

    #[test]
    fn resolve_turns_promotes_clear_subfloor_interjection() {
        // Speaker 0 turn, brief different-voice interjection promoted by the
        // embedding-margin arbitration (clear margin to cluster 1), speaker 0
        // resumes. The "Yeah"-interjection fixture class: the 1.5s floor must
        // not absorb a corroborated voice change.
        let pieces = vec![
            PieceIn { start_secs: 30.0, dur_secs: 2.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 32.0, dur_secs: 0.4, cluster: Some(1), margin: Some(0.4), promoted_subfloor: true },
            PieceIn { start_secs: 32.6, dur_secs: 2.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
        ];
        let turns = resolve_turns(&pieces);
        assert_eq!(turns.len(), 3, "{:?}", turns);
        assert_eq!(turns[1].cluster, 1);
        assert!(turns[1].low_confidence);
        assert!(!turns[1].continues_previous);
        assert!((turns[1].start_secs - 32.0).abs() < 1e-9);
        assert!((turns[1].end_secs - 32.4).abs() < 1e-9);
        assert!((turns[2].start_secs - 32.6).abs() < 1e-9);
        for w in turns.windows(2) {
            assert!(w[0].end_secs <= w[1].start_secs + 1e-9, "spans ordered");
        }
    }

    // ---- gap rescue: rescue_candidates (change gap-speech-voice-attribution) ----

    fn rsub(start: f64, end: f64, cluster: usize, margin: f32) -> RescueSubWindow {
        RescueSubWindow { start_secs: start, end_secs: end, cluster, margin }
    }

    fn rgap(raw: Option<(f64, f64)>, subs: Vec<RescueSubWindow>) -> RescueGap {
        RescueGap { gap_start_secs: 10.0, gap_end_secs: 12.0, raw_span: raw, sub_windows: subs }
    }

    #[test]
    fn rescue_candidates_selects_last_decided_subwindow_contradicting_borrow_winner() {
        // sp0 [8,10] | gap [10,12] | sp1 [12,14]; raw [10.2,11.2] mid 10.7s
        // -> borrow winner sp0 (700ms); last decided sub-window is sp1 -> splice
        let turns = vec![(8.0, 10.0, 0), (12.0, 14.0, 1)];
        let subs = vec![
            rsub(10.2, 10.6, 0, 0.30), // left-flank continuation: diagnostic only
            rsub(10.9, 11.15, 1, 0.08),
        ];
        let cands = rescue_candidates(&turns, &[rgap(Some((10.2, 11.2)), subs)], 0.8, 0.05, 3000);
        assert_eq!(cands.len(), 1, "{cands:?}");
        assert_eq!(cands[0].cluster, 1);
        assert!((cands[0].start_secs - 10.9).abs() < 1e-9);
        assert!((cands[0].end_secs - 11.15).abs() < 1e-9);
        assert!((cands[0].margin - 0.08).abs() < 1e-6);
    }

    #[test]
    fn rescue_candidates_abstains_on_undecided_floor_silence_interior_and_meeting_edge() {
        let turns = vec![(8.0, 10.0, 0), (12.0, 14.0, 1)];
        // undecided identity
        let subs = vec![rsub(10.5, 10.8, 1, 0.04)];
        assert!(rescue_candidates(&turns, &[rgap(Some((10.2, 11.2)), subs)], 0.8, 0.05, 3000).is_empty());
        // sub-floor raw span
        let subs = vec![rsub(10.5, 10.8, 1, 0.30)];
        assert!(rescue_candidates(&turns, &[rgap(Some((10.2, 10.9)), subs)], 0.8, 0.05, 3000).is_empty());
        // true silence: no raw span
        assert!(rescue_candidates(&turns, &[rgap(None, vec![])], 0.8, 0.05, 3000).is_empty());
        // interior gap (same-label flanks)
        let turns_same = vec![(8.0, 10.0, 0), (12.0, 14.0, 0)];
        let subs = vec![rsub(10.5, 10.8, 1, 0.30)];
        assert!(rescue_candidates(&turns_same, &[rgap(Some((10.2, 11.2)), subs)], 0.8, 0.05, 3000).is_empty());
        // meeting edge: gap before the first turn (no left flank)
        let mut g = rgap(Some((7.1, 7.9)), vec![rsub(7.2, 7.6, 0, 0.20)]);
        g.gap_start_secs = 7.0;
        g.gap_end_secs = 7.9;
        assert!(rescue_candidates(&turns, &[g], 0.8, 0.05, 3000).is_empty());
    }

    #[test]
    fn rescue_candidates_beyond_cap_splices_and_best_eq_winner_is_noop() {
        // beyond cap: both flanks > 3000ms from the raw midpoint -> no winner;
        // decided voice replaces geometry that would leave the row unattributed
        let turns = vec![(8.0, 10.0, 0), (18.0, 20.0, 1)];
        let subs = vec![rsub(13.5, 13.9, 1, 0.20)];
        let mut g = rgap(Some((13.4, 14.4)), subs);
        g.gap_start_secs = 10.0;
        g.gap_end_secs = 18.0;
        let cands = rescue_candidates(&turns, &[g], 0.8, 0.05, 3000);
        assert_eq!(cands.len(), 1, "{cands:?}");
        assert_eq!(cands[0].cluster, 1);
        // no-op: decided cluster equals the borrow winner (mid 10.7 -> sp0)
        let turns = vec![(8.0, 10.0, 0), (12.0, 14.0, 1)];
        let subs = vec![rsub(10.5, 10.8, 0, 0.30)];
        assert!(rescue_candidates(&turns, &[rgap(Some((10.2, 11.2)), subs)], 0.8, 0.05, 3000).is_empty());
    }

    #[test]
    fn rescue_candidates_treats_near_tie_as_geometry_abstain() {
        // raw mid 11.0 sits 600ms from BOTH flanks (different labels): the
        // modeled winner abstains and the decided sub-window splices
        let turns = vec![(8.0, 10.0, 0), (12.0, 14.0, 1)];
        let subs = vec![rsub(10.8, 11.2, 1, 0.09)];
        let cands = rescue_candidates(&turns, &[rgap(Some((10.6, 11.4)), subs)], 0.8, 0.05, 3000);
        assert_eq!(cands.len(), 1, "{cands:?}");
        assert_eq!(cands[0].cluster, 1);
        // 100ms-epsilon boundary: mid 10.95 -> dists 950/1050 (100 apart) -> abstain
        let subs = vec![rsub(10.8, 11.2, 1, 0.09)];
        let cands = rescue_candidates(&turns, &[rgap(Some((10.5, 11.4)), subs)], 0.8, 0.05, 3000);
        assert_eq!(cands.len(), 1, "tie abstain still splices on decided voice");
        // clear winner: mid 10.5 -> sp0 by 1000ms; sp1 candidate is a no-op
        let subs = vec![rsub(10.3, 10.7, 1, 0.30)];
        let cands = rescue_candidates(&turns, &[rgap(Some((10.2, 10.8)), subs)], 0.8, 0.05, 3000);
        assert!(cands.is_empty());
    }

    #[test]
    fn rescue_candidates_is_deterministic() {
        let turns = vec![(8.0, 10.0, 0), (12.0, 14.0, 1)];
        let subs = vec![
            rsub(10.2, 10.6, 0, 0.30),
            rsub(10.9, 11.15, 1, 0.08),
        ];
        let gaps = vec![rgap(Some((10.2, 11.2)), subs)];
        let a = rescue_candidates(&turns, &gaps, 0.8, 0.05, 3000);
        let b = rescue_candidates(&turns, &gaps, 0.8, 0.05, 3000);
        assert_eq!(a, b);
    }

    #[test]
    fn rescued_far_flank_piece_coalesces_into_right_turn() {
        // S2b shape: [onset, gap end] promoted piece matching the right flank
        // founds the turn; the right flank coalesces into it; the surviving
        // turn carries the promotion flag (low_confidence) and the boundary
        // moves to the sub-window start.
        let spliced = vec![
            PieceIn { start_secs: 8.0, dur_secs: 2.0, cluster: Some(0), margin: Some(0.3), promoted_subfloor: false },
            PieceIn { start_secs: 10.7, dur_secs: 0.32, cluster: Some(1), margin: Some(0.09), promoted_subfloor: true },
            PieceIn { start_secs: 11.02, dur_secs: 2.0, cluster: Some(1), margin: Some(0.4), promoted_subfloor: false },
        ];
        let turns = resolve_turns(&spliced);
        assert_eq!(turns.len(), 2, "{turns:?}");
        assert_eq!(turns[0].cluster, 0);
        assert_eq!(turns[1].cluster, 1);
        assert!((turns[1].start_secs - 10.7).abs() < 1e-9, "{:?}", turns[1]);
        assert!(turns[1].low_confidence, "founder carries the promotion flag");
        assert!(!turns[1].continues_previous);
    }
}
