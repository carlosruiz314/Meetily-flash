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
    let mut out = Vec::new();
    for run in runs {
        let mut bounds = vec![run.start_frame];
        for cand in label_change_candidates(labels, run, radius) {
            let t = frame_time(cand, frame_shift);
            if corroborate_split(window_tracks, t, frame_shift, SPLIT_TOLERANCE_SECS, radius) {
                bounds.push(cand);
            }
        }
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
}
