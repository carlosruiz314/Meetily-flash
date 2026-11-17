//! RED tests for the net-worse guard's alignment statistic
//! (overlap-repair-net-worse-guard task 1.1).
//!
//! The statistic compares a promoted repair render against the replaced
//! mixture rows, scoped to tokens OUTSIDE the seed span (the conservation
//! set), aligned PER BADGE: the repair render is two whole-window stream
//! rows whose tokens carry no timestamps, and a global ordered alignment
//! of their concatenation against time-interleaved baseline rows caps near
//! half even for a perfect separated repair (panel finding, task 1.1) —
//! so per-badge resolution is load-bearing and pinned here.
//!
//! All token arrays are invented generics (the run_assembly test style) —
//! never captured real sequences.

use app_lib::audio::speaker::net_worse_guard::{
    BaselineRow, ConservationGroup, RepairStream, SeedWalls, align_groups,
    build_conservation_groups, normalize_tokens,
};

fn group(badge: &str, text: &str) -> ConservationGroup {
    ConservationGroup {
        badge: badge.to_string(),
        baseline_tokens: normalize_tokens(text),
    }
}

fn stream(badge: &str, text: &str) -> RepairStream {
    RepairStream {
        badge: badge.to_string(),
        tokens: normalize_tokens(text),
    }
}

fn recall_for(groups: &[ConservationGroup], streams: &[RepairStream], badge: &str) -> f64 {
    align_groups(groups, streams)
        .into_iter()
        .find(|g| g.badge == badge)
        .unwrap_or_else(|| panic!("no alignment outcome for badge {badge}"))
        .recall
}

/// A repair that carries every conserved token, in order, scores 1.0.
#[test]
fn perfect_repair_conserves_fully() {
    let groups = vec![group("A", "alpha beta gamma delta epsilon")];
    let streams = vec![stream(
        "A",
        "prefix filler alpha beta gamma delta epsilon trailing words",
    )];
    let r = recall_for(&groups, &streams, "A");
    assert!((r - 1.0).abs() < 1e-9, "expected recall 1.0, got {r}");
}

/// The attested mangle shape: the repair substitutes fluent but different
/// tokens for part of the conserved text while every existing signal passes.
/// Recall must collapse below any sane bar.
#[test]
fn fluent_substitution_collapses_recall() {
    // Baseline tail substituted wholesale: 3 of 8 conserved tokens survive.
    let groups = vec![group(
        "A",
        "one two three four five six seven eight",
    )];
    let streams = vec![stream(
        "A",
        "one two three utterly different fluent words instead of the rest",
    )];
    let r = recall_for(&groups, &streams, "A");
    assert!(r <= 0.5, "substitution must collapse recall, got {r}");
}

/// Word-order scramble: the same token MULTISET in the wrong order must
/// fail the ordered statistic — a multiset comparison alone does NOT pass.
#[test]
fn word_order_scramble_fails_ordered_statistic() {
    let baseline = "one two three four five six seven eight";
    let scrambled = "eight seven six five four three two one";
    let groups = vec![group("A", baseline)];
    let streams = vec![stream("A", scrambled)];

    // Sanity: the multiset is identical (a bag-of-tokens guard would pass).
    let mut b: Vec<_> = normalize_tokens(baseline);
    let mut s: Vec<_> = normalize_tokens(scrambled);
    b.sort();
    s.sort();
    assert_eq!(b, s, "fixture must be multiset-equal");

    let r = recall_for(&groups, &streams, "A");
    assert!(r <= 0.4, "order scramble must degrade recall, got {r}");
}

/// THE PANEL FIXTURE: the baseline alternates badges in time order while
/// the repair render is two whole-window streams (each badge's words
/// contiguous in its own stream). Per-badge alignment scores ≈ 1.0; the
/// GLOBAL alignment of the concatenated streams against the interleaved
/// baseline caps near half even though the repair is PERFECT — pinning why
/// global alignment is rejected.
#[test]
fn per_badge_alignment_survives_interleaving_global_does_not() {
    // Time-ordered mixture rows alternate voices: a1 b1 a2 b2 ... a10 b10.
    let n = 10;
    let time_ordered: Vec<String> = (1..=n)
        .flat_map(|i| vec![format!("a{i}"), format!("b{i}")])
        .collect();
    // Per-badge groups extract each voice's tokens in time order; the
    // streams carry them contiguously.
    let groups = vec![
        ConservationGroup {
            badge: "A".to_string(),
            baseline_tokens: (1..=n).map(|i| format!("a{i}")).collect(),
        },
        ConservationGroup {
            badge: "B".to_string(),
            baseline_tokens: (1..=n).map(|i| format!("b{i}")).collect(),
        },
    ];
    let streams = vec![
        stream("A", "a1 a2 a3 a4 a5 a6 a7 a8 a9 a10"),
        stream("B", "b1 b2 b3 b4 b5 b6 b7 b8 b9 b10"),
    ];
    for badge in ["A", "B"] {
        let r = recall_for(&groups, &streams, badge);
        assert!(r >= 0.95, "per-badge recall must be ~1.0 for {badge}, got {r}");
    }

    // Global alignment: LCS of the interleaved baseline against the
    // concatenated streams. The best common subsequence takes a run of
    // a-tokens then all remaining b-tokens — ≈ half, because after any
    // b-token no further a-token can match (they precede the b-run in the
    // stream order).
    let global_stream: Vec<String> = streams.iter().flat_map(|s| s.tokens.clone()).collect();
    let lcs = app_lib::audio::speaker::net_worse_guard::lcs_length(&time_ordered, &global_stream);
    let ratio = lcs as f64 / time_ordered.len() as f64;
    assert!(
        ratio <= 0.65,
        "global alignment must cap near half ({ratio}) — if it scores high, \
         the interleaving fixture is wrong"
    );
}

/// Within-badge re-partitioning (rows re-cut between badges' words) costs
/// nothing: the statistic works on per-badge concatenated tokens and is
/// boundary-blind by construction.
#[test]
fn repartitioning_is_boundary_blind() {
    let groups = vec![group("A", "alpha beta gamma delta")];
    let streams = vec![stream("A", "alpha beta gamma delta")];
    let r = recall_for(&groups, &streams, "A");
    assert!((r - 1.0).abs() < 1e-9, "re-partitioning must not cost recall, got {r}");
}

/// Repair-side tokens outside every conservation group (the seed span's
/// words, unscopable without timestamps) cost nothing: recall is the
/// denominator-bounded side. Precision still records the extra mass.
#[test]
fn unscoped_repair_tokens_cost_nothing() {
    let groups = vec![group("A", "alpha beta gamma")];
    let streams = vec![stream(
        "A",
        "seedonly1 seedonly2 seedonly3 alpha beta gamma seedonly4",
    )];
    let outcomes = align_groups(&groups, &streams);
    let g = &outcomes[0];
    assert!((g.recall - 1.0).abs() < 1e-9, "recall must ignore unscoped tokens, got {}", g.recall);
    assert!(
        g.precision < 0.7,
        "precision must record the extra mass, got {}",
        g.precision
    );
}

/// Normalization: case and punctuation never block a match; tokens that
/// normalize to nothing are dropped.
#[test]
fn normalization_strips_case_and_punctuation() {
    let groups = vec![group("A", "Hello, World! It's fine.")];
    let streams = vec![stream("A", "hello world its fine")];
    let r = recall_for(&groups, &streams, "A");
    assert!((r - 1.0).abs() < 1e-9, "normalization must match, got {r}");
    assert_eq!(normalize_tokens("... !!! ,,,").len(), 0);
}

/// A badge flip on conserved text: the conserved tokens land in the wrong
/// stream's alignment — BOTH groups degrade below any sane bar.
#[test]
fn badge_flip_degrades_both_groups() {
    let groups = vec![
        group("A", "alpha beta gamma delta"),
        group("B", "one two three four"),
    ];
    // Streams carry the right tokens but under swapped badges.
    let streams = vec![
        stream("A", "one two three four"),
        stream("B", "alpha beta gamma delta"),
    ];
    for badge in ["A", "B"] {
        let r = recall_for(&groups, &streams, badge);
        assert!(r <= 0.3, "badge flip must degrade {badge}, got {r}");
    }
}

/// A conservation group whose badge resolves to NO stream is a hard miss
/// (recall 0.0), never a silent skip — a vanished voice is net-worse.
#[test]
fn group_without_stream_scores_zero() {
    let groups = vec![group("A", "alpha beta gamma")];
    let streams = vec![stream("B", "unrelated tokens entirely")];
    let outcomes = align_groups(&groups, &streams);
    let g = &outcomes[0];
    assert!((g.recall - 0.0).abs() < 1e-9, "missing stream must score 0.0, got {}", g.recall);
}

/// A group with an empty baseline carries no conserved text: it produces
/// NO outcome (the guard's empty-conservation-set rule lives above this
/// layer and refuses the promotion — task 2.1).
#[test]
fn empty_baseline_group_is_skipped() {
    let groups = vec![group("A", "... !!!")];
    let streams = vec![stream("A", "whatever tokens")];
    let outcomes = align_groups(&groups, &streams);
    assert!(outcomes.is_empty(), "empty baseline must emit no outcome");
}

// ---------------------------------------------------------------------------
// Conservation-set scoping (baseline-side, design D1) — RED tests.
// The guard scopes on the BASELINE side because repair-side tokens carry no
// per-token timestamps: rows fully outside the seed span are conserved
// whole, a row straddling a wall is partitioned proportionally by word
// count at the wall (the replacement-scope precedent), inside rows are
// excluded.
// ---------------------------------------------------------------------------

fn brow(badge: &str, start: f64, end: f64, text: &str) -> BaselineRow {
    BaselineRow {
        badge: badge.to_string(),
        start_s: start,
        end_s: end,
        text: text.to_string(),
    }
}

fn walls(start: f64, end: f64) -> SeedWalls {
    SeedWalls {
        start_s: start,
        end_s: end,
    }
}

/// A row entirely before (or after) the seed span is conserved whole.
#[test]
fn row_fully_outside_is_conserved_whole() {
    let rows = vec![brow("A", 0.0, 4.0, "alpha beta gamma delta")];
    let groups = build_conservation_groups(&rows, &walls(5.0, 9.0), &walls(0.0, 100.0));
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].baseline_tokens, normalize_tokens("alpha beta gamma delta"));
}

/// A row fully inside the seed span is the thing being fixed — excluded.
#[test]
fn row_fully_inside_is_excluded() {
    let rows = vec![brow("A", 5.5, 8.5, "alpha beta gamma delta")];
    let groups = build_conservation_groups(&rows, &walls(5.0, 9.0), &walls(0.0, 100.0));
    assert!(groups.is_empty(), "inside rows must contribute nothing");
}

/// A row straddling a wall is split proportionally by word count at the
/// wall; only the outside piece is conserved. 0-10s row, wall at 5.0s:
/// half the words survive as the head piece.
#[test]
fn straddling_row_splits_proportionally_at_the_wall() {
    let rows = vec![brow(
        "A",
        0.0,
        10.0,
        "w1 w2 w3 w4 w5 w6 w7 w8",
    )];
    let groups = build_conservation_groups(&rows, &walls(5.0, 20.0), &walls(0.0, 100.0));
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].baseline_tokens, normalize_tokens("w1 w2 w3 w4"));
}

/// A row spanning the whole seed span keeps BOTH outside pieces.
#[test]
fn row_spanning_the_whole_seed_conserves_both_pieces() {
    let rows = vec![brow(
        "A",
        0.0,
        10.0,
        "h1 h2 h3 h4 m1 m2 m3 m4 t1 t2 t3 t4",
    )];
    let groups = build_conservation_groups(&rows, &walls(4.0, 6.0), &walls(0.0, 100.0));
    assert_eq!(groups.len(), 1);
    // Proportional rounding: wall 4.0/10 → 0.4*12 = 4.8 → head = first 5
    // tokens; wall 6.0/10 → 0.6*12 = 7.2 → tail = last 5. The boundary
    // fuzz is inherent to word-proportional partition (the live
    // replacement-scope precedent has the same property).
    assert_eq!(
        groups[0].baseline_tokens,
        normalize_tokens("h1 h2 h3 h4 m1 m4 t1 t2 t3 t4")
    );
}

/// Edge-touching rows (end == seed start, start == seed end) do not
/// overlap — conserved whole, no proportional slicing.
#[test]
fn edge_touching_rows_are_conserved_whole() {
    let rows = vec![
        brow("A", 0.0, 5.0, "alpha beta gamma"),
        brow("B", 9.0, 12.0, "one two three"),
    ];
    let groups = build_conservation_groups(&rows, &walls(5.0, 9.0), &walls(0.0, 100.0));
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].baseline_tokens, normalize_tokens("alpha beta gamma"));
    assert_eq!(groups[1].baseline_tokens, normalize_tokens("one two three"));
}

/// Groups accumulate per badge in first-appearance order.
#[test]
fn groups_are_built_per_badge_in_first_appearance_order() {
    let rows = vec![
        brow("B", 0.0, 2.0, "b-words here"),
        brow("A", 2.0, 4.0, "a-words here"),
        brow("B", 10.0, 12.0, "more b words"),
    ];
    let groups = build_conservation_groups(&rows, &walls(5.0, 9.0), &walls(0.0, 100.0));
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].badge, "B");
    assert_eq!(groups[0].baseline_tokens, normalize_tokens("b-words here more b words"));
    assert_eq!(groups[1].badge, "A");
}

/// Rows are clipped to the WINDOW walls before seed-scoping: a promoted
/// window replaces only the rows' within-window text (beyond-wall text
/// persists as outside pieces), so out-of-window tokens are not conserved.
#[test]
fn rows_are_clipped_to_the_window_before_seed_scoping() {
    // 20 tokens across 0-20s: token i covers [i-1, i) seconds.
    let text: String = (1..=20)
        .map(|i| format!("t{i} "))
        .collect::<String>()
        .trim_end()
        .to_string();
    let rows = vec![brow("A", 0.0, 20.0, &text)];
    // Window [5,10] → tokens t6..t10; seed [8,9] → t9 excluded; the row
    // spans the whole seed → both outside pieces conserved: t6 t7 t8 + t10.
    let groups = build_conservation_groups(&rows, &walls(8.0, 9.0), &walls(5.0, 10.0));
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].baseline_tokens, vec!["t6", "t7", "t8", "t10"]);
}

/// A row entirely beyond the window contributes nothing.
#[test]
fn row_outside_the_window_is_excluded() {
    let rows = vec![brow("A", 12.0, 20.0, "alpha beta gamma")];
    let groups = build_conservation_groups(&rows, &walls(8.0, 9.0), &walls(5.0, 10.0));
    assert!(groups.is_empty());
}
