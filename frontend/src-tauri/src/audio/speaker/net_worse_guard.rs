//! The net-worse guard's alignment statistic
//! (overlap-repair-net-worse-guard, design D2).
//!
//! Principle: the repair may only change what is broken — text the
//! promotion was not asked to fix (tokens outside the seed span) must
//! survive into the repair render. This module is the pure decision core:
//! no I/O, no models, no fixtures (hexagonal: use-case layer, std only).
//!
//! Why PER BADGE (panel finding, 2026-11-17): the repair render is two
//! whole-window stream rows whose tokens carry no timestamps, and a global
//! ordered alignment of their concatenation against time-interleaved
//! baseline rows caps near half even for a perfect separated repair —
//! no bar could then separate good repairs from the attested mangle. Each
//! badge's conserved tokens are therefore aligned against the stream whose
//! vote evidence resolves to that badge, and the ratio is RECALL (LCS over
//! the conserved count) so repair-side tokens outside every group — the
//! seed span's words, unscopable without timestamps — cost nothing.
//! Precision is recorded alongside recall as diagnostic mass.

/// Normalize transcript text into comparable tokens: whitespace-split,
/// lowercased, punctuation stripped, empties dropped. Unicode-aware
/// (ES/CA accents survive).
pub fn normalize_tokens(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|tok| {
            tok.to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
        })
        .filter(|tok| !tok.is_empty())
        .collect()
}

/// Classic order-preserving LCS length (rolling single-row DP).
pub fn lcs_length(a: &[String], b: &[String]) -> usize {
    if a.is_empty() || b.is_empty() {
        return 0;
    }
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = vec![0usize; b.len() + 1];
    for av in a {
        for (j, bv) in b.iter().enumerate() {
            cur[j + 1] = if av == bv {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            };
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// One badge's conserved baseline: the ordered normalized tokens of the
/// replaced mixture rows OUTSIDE the seed span's walls (whole rows when
/// fully outside; the outside piece of a proportionally split straddling
/// row). Scoping happens on the BASELINE side — the caller (the guard,
/// task 2.x) builds these from the meeting's own mixture rows.
pub struct ConservationGroup {
    pub badge: String,
    pub baseline_tokens: Vec<String>,
}

/// One repair stream: its vote-resolved badge and its normalized tokens.
pub struct RepairStream {
    pub badge: String,
    pub tokens: Vec<String>,
}

/// One conservation group's alignment outcome.
pub struct GroupAlignment {
    pub badge: String,
    /// LCS length over the group's conserved count — 1.0 means fully
    /// conserved; the guard's bar (settled offline, task 1.5) applies here.
    pub recall: f64,
    /// LCS length over the stream's token count — diagnostic mass (extra
    /// repair-side tokens: seed-span words, hallucination).
    pub precision: f64,
    pub lcs_len: usize,
}

/// Align every conservation group against the stream its badge resolves
/// to. A group with an empty baseline emits NO outcome (there is nothing
/// to conserve; the empty-conservation-set refusal lives in the guard).
/// A badge resolving to NO stream is a HARD MISS (recall 0.0, never a
/// silent skip): a vanished voice is net-worse.
pub fn align_groups(groups: &[ConservationGroup], streams: &[RepairStream]) -> Vec<GroupAlignment> {
    let mut out = Vec::with_capacity(groups.len());
    for g in groups {
        if g.baseline_tokens.is_empty() {
            continue;
        }
        let stream = streams.iter().find(|s| s.badge == g.badge);
        let (lcs, precision) = match stream {
            Some(s) if !s.tokens.is_empty() => {
                let lcs = lcs_length(&g.baseline_tokens, &s.tokens);
                (lcs, lcs as f64 / s.tokens.len() as f64)
            }
            // A present-but-empty stream conserves nothing: full miss.
            _ => (0, 0.0),
        };
        out.push(GroupAlignment {
            badge: g.badge.clone(),
            recall: lcs as f64 / g.baseline_tokens.len() as f64,
            precision,
            lcs_len: lcs,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Conservation-set scoping (baseline-side — design D1).
// ---------------------------------------------------------------------------

/// One replaced mixture row: its rendered badge, walls, and text.
pub struct BaselineRow {
    pub badge: String,
    pub start_s: f64,
    pub end_s: f64,
    pub text: String,
}

/// The seed span's walls — the only thing the promotion was asked to fix.
pub struct SeedWalls {
    pub start_s: f64,
    pub end_s: f64,
}

/// Build the conservation groups from the replaced mixture rows. Rows
/// fully outside the seed span are conserved whole under their badge; a
/// row straddling a wall is partitioned proportionally by word count at
/// the wall (the replacement-scope precedent) and only its outside
/// piece(s) are conserved; rows fully inside are excluded. Groups
/// accumulate per badge in first-appearance order, tokens in time order.
pub fn build_conservation_groups(rows: &[BaselineRow], seed: &SeedWalls) -> Vec<ConservationGroup> {
    let mut order: Vec<String> = Vec::new();
    let mut acc: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for row in rows {
        let tokens = normalize_tokens(&row.text);
        // Open-interval overlap: an edge-touching row does not overlap.
        let overlaps = row.start_s < seed.end_s && row.end_s > seed.start_s;
        let mut pieces: Vec<Vec<String>> = Vec::new();
        if !overlaps {
            pieces.push(tokens.clone());
        } else {
            let dur = (row.end_s - row.start_s).max(f64::EPSILON);
            // West wall inside the row → head piece is outside the span.
            if seed.start_s > row.start_s && seed.start_s < row.end_s {
                let frac = ((seed.start_s - row.start_s) / dur).clamp(0.0, 1.0);
                let split = ((frac * tokens.len() as f64).round() as usize).min(tokens.len());
                if split > 0 {
                    pieces.push(tokens[..split].to_vec());
                }
            }
            // East wall inside the row → tail piece is outside the span.
            if seed.end_s > row.start_s && seed.end_s < row.end_s {
                let frac = ((seed.end_s - row.start_s) / dur).clamp(0.0, 1.0);
                let split = ((frac * tokens.len() as f64).round() as usize).min(tokens.len());
                if split < tokens.len() {
                    pieces.push(tokens[split..].to_vec());
                }
            }
        }
        if pieces.is_empty() {
            continue; // fully inside — the thing being fixed
        }
        if !acc.contains_key(&row.badge) {
            order.push(row.badge.clone());
        }
        let slot = acc.entry(row.badge.clone()).or_default();
        for p in pieces {
            slot.extend(p);
        }
    }
    order
        .into_iter()
        .map(|badge| ConservationGroup {
            baseline_tokens: acc.remove(&badge).unwrap_or_default(),
            badge,
        })
        .collect()
}
