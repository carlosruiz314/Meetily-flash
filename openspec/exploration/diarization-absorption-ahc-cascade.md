# Exploration: Speaker Absorption — AHC Contamination-Cascade Hypothesis

> **Status:** Exploration findings (explore mode). Not a proposal yet.
> **Date:** 2026-07-03
> **Context:** `/opsx:explore "why did fingerprinting fail in that meeting?"`
> → narrowed to "which clustering stage absorbs UserB, and by what mechanism?"
> **Capability:** `openspec/specs/speaker-diarization/spec.md`
> **Production repro:** meeting `meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323`
>   (2026-06-22, 3 speakers, 70 min; one speaker "UserB" absorbed from ~min 30)

---

> ## RESULTS (instrumented diagnostic run, 2026-07-03)
>
> The contamination-cascade hypothesis is **REFUTED** by data. The instrumentation
> (Tract 2: AHC merge-tree log, committed in `commands.rs:test_cde5c264_embedding_drift_diagnostic`)
> ran against the production recording and produced:
>
> | Measurement | Value | Verdict |
> |---|---|---|
> | Merges into UserB's cluster | 17 | — |
> | UserA-origin members absorbed | **0** (all 17 merges) | **No contamination** |
> | Contamination onset step | `None` (never occurs) | Cascade mechanism absent |
> | UserB centroid drift (cos→her early) | **0.9995** | Centroid is stable, not drifting |
> | UserB centroid vs UserA (cos) | **0.3149** | Her centroid is far from UserA |
>
> UserB's cluster is **clean**: it never absorbs a UserA-origin chunk, and its
> centroid barely moves. The cascade (positive feedback via centroid drift) **does not
> occur**. The original hypothesis below is preserved as history but is dead.
>
> **Tract 1b — cap (Stage D) EXONERATED:** simulated `enforce_max_speakers_cap(3)` on
> the 5 smoothed clusters. UserB **SURVIVES** the cap (final clusters `[5, 1, 2]`).
> The cap fires twice (5→4→3): step 0 merges singleton cluster 4 into cluster 0; step 1
> merges cluster 0 (the dominant cluster, nn_sim=0.327) **into cluster 1 (UserB)** —
> i.e. UserB *absorbs* the dominant cluster, not the reverse. The cap is not the
> absorption mechanism. (Caveat: this is a chunk-level simulation; production
> `enforce_max_speakers_cap` operates on post-coalesce segment centroids, so the exact
> merge targets may differ — but UserB's centroid is cos 0.31 from UserA, the most
> distinct in the meeting, so she is the *least* likely cluster to be merged by the
> "most-isolated" heuristic under any label space.)
>
> **Tract 1c — the 178 "nearer UserB" chunks are 96 % sub-threshold noise:**
>
> | cos→UserB (absolute) | count | interpretation |
> |---|---|---|
> | < 0.2 | **87** | far — not her voice at the embedding level |
> | 0.2–0.3 | **82** | far — ambiguous orphan |
> | 0.3–0.4 | 2 | below merge threshold (0.40) |
> | 0.4–0.5 | 0 | — |
> | ≥ 0.5 | **7** | genuinely her voice (same-speaker range) |
>
> Only **7** of 178 are genuinely UserB's voice (cos ≥ 0.5) — and those 7 **ARE** in
> her cluster. The other **171** (96 %) sit below the 0.40 merge threshold; 87 of them
> are below 0.2. They are "nearer UserB than UserA" only in *relative* terms — in
> absolute terms they are far from both centroids and are assigned to UserA's dominant
> cluster purely by nearest-centroid tiebreak.
>
> **INVESTIGATION REOPENED 2026-07-03 (the "illusory" verdict above was premature — read
> the correction below before relying on it).** The Tract 1c histogram only compared each
> chunk against **two** of the three centroids (UserB vs UserA); it never checked
> **cluster 0** (the third speaker). Tracing where the 178 "nearer-UserB" chunks actually
> land by label: 7 are in UserB's cluster 1, 1 is in UserA's cluster 2, and **~170 are in
> cluster 0** (the third speaker). So the production absorption is **into the third
> speaker's cluster, not UserA's** — the entire "UserA over-merged UserB" framing was
> an artifact of a two-centroid comparison. The "illusory" conclusion also committed the
> ground-truth inference error ([[dont-infer-ground-truth-from-diarization]]): the user was
> in the meeting and confirms UserB spoke throughout; embeddings at cos 0.2–0.3 to her
> centroid are the **bug**, not evidence she went quiet. The real question is **why
> UserB's late-speech embeddings collapse from cos 0.85 (her 7 captured late chunks) to
> cos 0.2–0.3 (the ~170 orphans in cluster 0)** — an upstream failure (audio, chunking, or
> embedder under late-meeting acoustic conditions), not a clustering issue. Next measurement
> = Tract 3 (below): per-cluster embedding stability over time + where the 170 orphans sit
> relative to cluster 0's own centroid.

---

## TL;DR

~~The leading hypothesis is a contamination cascade~~ (REFUTED 2026-07-03 — see RESULTS
above). The doc is preserved as a record of the hypothesis + the instrumentation that
disproved it, and of the new lead (the `max_speakers` cap fires and is unmeasured).

**What is still established:** the absorption's origin **stage** is Stage A — the
existing `#[ignore]` diagnostic calls raw `cluster_by_centroids` with no smoothing and
no cap, and the absorption (137 early → 7 late) is present there. But the AHC's
*mechanism* is NOT contamination (0 UserA-origin merges, centroid stable at cos 0.9995).

**What changed (investigation REOPENED 2026-07-03):** the "closed / illusory" verdict was
premature — it inferred ground truth from the embedding geometry (violating
[[dont-infer-ground-truth-from-diarization]]) and, critically, the Tract 1c histogram
only compared chunks against UserB vs UserA, never against **cluster 0 (the third
speaker)**. Tracing the 178 "nearer-UserB" chunks by label: ~170 are in cluster 0, not
UserA. The production absorption is into the third speaker. The cascade (refuted) and the
cap (exonerated) findings stand, but neither was ever the mechanism. The open question is
now upstream: **why do UserB's late embeddings degrade from cos 0.85 to 0.2–0.3**, and
can the audio/chunking/embedder layer be fixed so her late speech is captured at all?

---

## Background (already established — do NOT re-derive)

Carried from the prior investigation + `memory/project_diarization_root_cause.md`:

| Claim | Evidence |
|---|---|
| Embeddings did NOT drift | UserB's OWN 7 late chunks are cos **0.8453** to her early centroid (same-speaker range). The 0.2253 figure was a misread of the mean cosine of ALL late chunks (low only because most belong to other speakers). |
| Geometry is characterized | ~178/430 late chunks are cosine-nearer to UserB's early centroid than to UserA's, yet her cluster kept only 7. |
| Fingerprinting / cross-meeting registry cannot recover this | cde5c264 was UserB's FIRST recorded meeting; Step-5 searches per contaminated cluster centroid, not per segment; the registry keys by ordinal `"Speaker N"` (`speaker_id=None`), and `link_embedding_to_speaker` is dead code. |
| Flicker is FIXED | Temporal-coherence smoothing (`sherpa_adapter.rs:745 smooth_to_fixed_point`), shipped on `main` via `cdbaf54`. Flicker 1.6 %. **Out of scope here.** |
| Fix must be clustering-level | Because the loss originates in Stage A and the fingerprinting layer is structurally downstream of per-cluster centroids. |

The corrective change `diarization-absorption-scope-correction` (archived 2026-06-30,
on `main` as `ee28995`) corrected the canonical spec's false "absorbed speaker is
recovered mid-meeting" scenario and committed the diagnostic as characterization. It
explicitly did **not** determine the root cause.

---

## The four stages that can eat chunks

```
build_chunks → embeddings (FIXED; cos 0.85 proves embeddings are clean)
        ↓
[A] cluster_by_centroids        greedy AHC: merge highest-sim pair > threshold,
    sherpa_adapter.rs:498        duration-weighted centroid, recompute row/col a
        ↓ labels_A, centroids_A   ← diagnostic uses THESE; absorption present here
[B] smooth_to_fixed_point        ±W neighborhood vote, damped self-weight 0.6
    sherpa_adapter.rs:745         + exp(-|i-j|) neighbors + μ=0.03 margin
        ↓ labels_B                ← NOT measured in current diagnostic
[C] segment coalesce +           inside process(); merges adjacent same-label segs
    renumber_speakers
        ↓ segments_C
[D] enforce_max_speakers_cap     merge MOST-ISOLATED cluster into its NN (only fires
    commands.rs:856               if cluster count > effective cap)
        ↓ segments_D              ← NOT measured in current diagnostic
```

**Key fact:** the diagnostic (`commands.rs:1505`) calls raw
`cluster_by_centroids(&chunks, 0.40)` directly — **not** `adapter.process()` and **not**
`enforce_max_speakers_cap`. So every geometry figure in the Background table is Stage-A
output. Stages B and D are unmeasured but cannot be the *origin* (the loss is already 7
at Stage A); they can only compound.

---

## The contamination-cascade hypothesis

### The AHC mechanism (sherpa_adapter.rs:522-569)

```rust
loop {
    // find the globally-highest-similarity pair above threshold
    let mut best_sim = threshold;
    for a in 0..n { for b in (a+1)..n {
        if alive[a] && alive[b] && sim[a][b-a-1] > best_sim {
            best_sim = sim[a][b-a-1]; best_pair = Some((a,b));
        }
    }}
    let Some((a,b)) = best_pair else { break };

    // duration-weighted centroid merge: b into a
    let w_a = dur_a / (dur_a + dur_b);  let w_b = dur_b / (dur_a + dur_b);
    centroids[a] = centroids[a]*w_a + centroids[b]*w_b;
    // recompute similarities touching a
}
```

Properties of this loop that matter for absorption:

1. **Greedy, global.** The highest-similarity pair merges first, regardless of which
   speakers they belong to. Same-speaker chunks cluster first (high sim), building
   speaker-pure clusters. Cross-speaker merges happen later at lower similarity — but
   anything above `threshold = 0.40` is eligible.
2. **Single drifted centroid.** The cluster is represented by ONE vector, the
   duration-weighted mean of all members. Once a foreign chunk joins, the centroid
   moves toward that chunk's region.
3. **Positive feedback.** After the centroid drifts toward speaker X, the NEXT X-like
   chunk finds this cluster *more* similar than before the drift (the centroid moved
   closer to X). Each absorption lowers the bar for the next one.
4. **Asymmetry.** UserA's cluster (growing, many members) is barely moved by one
   foreign chunk. UserB's cluster (if small at contamination onset) is moved a lot.

### The proposed failure sequence for cde5c264

```
early meeting:  UserB's cluster grows cleanly (137 early chunks, pure).
                UserA's cluster grows cleanly.
                Third speaker present.

mid-meeting:    A UserA-like chunk has cos > 0.40 to UserB's centroid
                (contamination onset). It merges in.
                UserB's centroid shifts slightly toward UserA.

late meeting:   Each subsequent UserB late-chunk now compares against:
                  - UserB's DRIFTED centroid (shifted toward UserA)
                  - UserA's CLEAN centroid (large, stable)
                Some find UserA's centroid nearer → merge into UserA.
                Each such merge drifts UserB further toward UserA → cascade.
                Result: UserB's cluster keeps ~7 late chunks; UserA absorbs ~178
                chunks that are cosine-nearer UserB's EARLY (pre-drift) centroid.
```

The "178 nearer UserB's early centroid" geometry **does not contradict** this
hypothesis. The diagnostic measures proximity to UserB's **early** centroid
(recomputed from early chunks only). The cascade is driven by proximity to her
**drifted** centroid at merge time, which the diagnostic does not currently track.

### What would confirm vs refute the hypothesis

| Outcome of instrumentation | Verdict |
|---|---|
| Contamination onset is early; centroid drifts monotonically toward UserA after onset; post-onset absorbed members skew UserA-like. | **Confirmed** — cascade is the mechanism. Fix venue = a merge-purity guard / centroid re-anchor / size-aware threshold inside `cluster_by_centroids`. |
| Contamination onset is late or absent; centroid barely drifts; yet late chunks still prefer UserA. | **Refuted** — the loss is pure nearest-centroid geometry (UserB's late voice genuinely sits between the two centroids), and the fix is a different clustering approach (e.g., per-chunk re-assignment against fixed early centroids, or a different linkage). |
| Onset is early but centroid does not drift (shift ≈ 0). | **Refuted on the cascade mechanism** — points at the `max_speakers` cap or a threshold-floor issue instead. |

---

## Instrumentation sketch

Both tracts are **read-only, `#[ignore]`, no production-code change**. They extend the
existing `test_cde5c264_embedding_drift_diagnostic` (or sit alongside it). Tract 2 is
the high-value one; Tract 1 is a cheap addition to exonerate B/D.

### Tract 1 — cross-stage stability check (cheap, ~15 min)

Add after the existing raw-AHC characterization to confirm whether smoothing (B) and the
cap (D) compound the absorption on top of Stage A:

```rust
// Full pipeline: process() = AHC + smoothing + coalesce (Stages A+B+C), no cap.
// adapter_arc is consumed by the earlier spawn_blocking; rebuild or Arc-clone.
let diar = tokio::task::spawn_blocking(move || {
    adapter2.process(&samples_arc2, DIARIZATION_SAMPLE_RATE, &segments_arc2)
}).await.expect("process");
let (mut centroids_full, segments_full) = (diar.centroids, diar.segments);

// Match full-pipeline clusters to Stage-A labels by centroid cosine (IDs are
// renumbered by renumber_speakers, so label-equality does not work). Compute
// the UserB-equivalent's late-chunk share: the surviving cluster with the
// smallest late-duration that had >=5 early chunks. Compare count to Stage A's 7.

// Then WITH the cap (Stage D):
let cap = resolve_effective_cap_for_meeting(&pool, meeting_id).await as usize;
enforce_max_speakers_cap(&mut centroids_full, &mut segments_full, cap);
// Re-count. Did the UserB-equivalent shrink further?
```

**Expected output:** a three-row table —
`Stage A (raw AHC): 7 | Stage A+B+C (process): ? | Stage A+B+C+D (full): ?`.
If B/D leave it at 7, they are exonerated and all effort goes to Tract 2. If they push
it lower, they compound and a separate (smaller) fix applies.

**Matching gotcha:** cluster IDs are renumbered after coalescing. Match by late-duration
share of the smallest surviving cluster, not by label equality.

### Tract 2 — AHC merge-tree instrumentation (the real prize)

Re-implement the AHC loop locally inside the diagnostic (same pattern as the sequential
prototype already at `commands.rs:1632-1688` — copy the loop, add logging). This does
NOT modify production `cluster_by_centroids`; it is a diagnostic-local copy.

```rust
// ---- Diagnostic-local instrumented copy of cluster_by_centroids ----
// Mirrors sherpa_adapter.rs:498-589 verbatim EXCEPT it records every merge.
struct MergeEvent {
    step: usize,
    surv: usize,          // surviving cluster index
    abso: usize,         // absorbed cluster index
    sim: f32,            // similarity that triggered the merge
    shift: f32,          // cos(centroid[surv] before, after) — drift magnitude
    absorbed_member_count: usize,
}

let mut merge_log: Vec<MergeEvent> = Vec::new();
// ... identical init: members, centroids, cluster_durations, alive, sim matrix ...

loop {
    // ... identical best-pair scan ...
    let Some((a, b)) = best_pair else { break };

    let cent_before = centroids[a].clone();
    let n_absorbed = members[b].len();
    // ... identical duration-weighted merge of b into a ...
    let shift = cosine(&cent_before, &centroids[a]);

    merge_log.push(MergeEvent {
        step: merge_log.len(), surv: a, abso: b, sim: best_sim,
        shift, absorbed_member_count: n_absorbed,
    });
}

// After the loop, identify UserB's cluster-ancestor: the surviving index
// whose `members` set overlaps the most with her Stage-A early-chunk set.
let cyn_anc = /* argmax over surviving indices of |members[i] ∩ cyn_early_chunks| */;

// Replay: every merge whose surv or abso == cyn_anc touched UserB's cluster.
let mut onset: Option<usize> = None;
for m in merge_log.iter().filter(|m| m.surv == cyn_anc || m.abso == cyn_anc) {
    // For the absorbed cluster's members, decide if they are "UserA-origin":
    //   is cos(member_emb, carlos_final_centroid) > cos(member_emb, cyn_final_centroid)?
    // If yes AND the merge is into cyn_anc, this is a contamination event.
    if onset.is_none() && /* first carlos-origin member absorbed into cyn_anc */ {
        onset = Some(m.step);
    }
    eprintln!("step {} merge into cyn: sim={:.3} shift={:.4} carlos_origin={}",
              m.step, m.sim, m.shift, /* bool */);
}
eprintln!("contamination onset at step {:?}", onset);

// Cumulative centroid drift: plot cos(cyn_centroid_at_step, cyn_early_centroid)
// across all steps >= onset. Monotonic decrease toward UserA = cascade signature.
```

**What this prints:**
- The **step** at which the first UserA-origin chunk entered UserB's cluster
  (contamination onset).
- **Per-merge centroid shift** (`shift` field) — how far each merge moved her centroid.
- **Cumulative drift** toward UserA from onset onward — monotonic = cascade.
- **Post-onset absorbed-member origin** — do the chunks joining UserB after onset skew
  UserA-like (cascade) or UserB-like (she's just small)?

---

## Candidate fix venues (ONLY if the cascade hypothesis is confirmed)

These are brainstormed directions for a future OpenSpec proposal — NOT to be implemented
from this doc. Each has its own trade-offs and needs a dedicated change.

1. **Merge-purity guard.** When considering a merge, reject if the candidate member's
   similarity to the cluster's *initial* seed is below a floor, even if it's above the
   running-centroid threshold. Prevents centroid drift from lowering the bar.
   - Risk: may fragment a speaker whose voice legitimately varies over 70 min.
2. **Centroid re-anchor.** Periodically recompute the centroid from the *high-confidence
   core* (members above a high sim to the current centroid) rather than the duration-
   weighted mean of all members. Resists contamination moving the centroid.
   - Risk: core-set definition is a new threshold; needs adversarial tests.
3. **Size-aware threshold.** Raise the merge threshold for small clusters (so a foreign
   chunk is less likely to contaminate a small pure cluster) — the asymmetry that makes
   small clusters vulnerable.
   - Risk: may prevent legitimate small-speaker formation.
4. **Two-pass: early-anchored re-assignment.** Run AHC once to get centroids; then
   re-assign every chunk to its nearest *stable* centroid (one with enough members),
   ignoring the drifted representation. This is the cleanest separation of "find
   speakers" from "assign chunks."
   - Risk: a new assignment pass; needs to handle the cold-start (no stable centroid
     yet for a speaker who only appears late).

None of these is committed. The instrumentation must run before any is scoped.

---

## Open questions

1. **Is contamination onset early or late in the merge sequence?** If early (when
   UserB's cluster is still small), the cascade hypothesis is viable — small clusters
   are most vulnerable to centroid drift. If late, the mechanism is different.
2. **Does UserB's centroid drift monotonically toward UserA, or is it stable?** This
   is the single most diagnostic measurement — cascade vs pure-geometry.
3. **Do smoothing (B) and the cap (D) compound the absorption on top of Stage A?**
   Tract 1 answers this. If they do, the fix spans two venues.
4. **Is the third speaker (not UserB, not UserA) involved?** cde5c264 has 3 speakers.
   The cascade might route through the third cluster as an intermediate. The
   instrumentation should tag origins against all final centroids, not just UserA's.
5. **Does the cascade reproduce on other multi-speaker meetings?** cde5c264 is one sample.
   Before scoping a fix, run the instrumentation on at least one other meeting with a
   known absorbed speaker (if such a recording exists in the prod DB).

---

## Cross-references

- **Canonical spec:** `openspec/specs/speaker-diarization/spec.md`
- **Corrective change (archived, on `main`):**
  `openspec/changes/archive/2026-06-30-diarization-absorption-scope-correction/design.md`
- **Diagnostic:** `frontend/src-tauri/src/audio/speaker/commands.rs:1449
  test_cde5c264_embedding_drift_diagnostic`
- **AHC core:** `frontend/src-tauri/src/audio/speaker/sherpa_adapter.rs:498
  cluster_by_centroids`
- **Smoothing:** `frontend/src-tauri/src/audio/speaker/sherpa_adapter.rs:745
  smooth_to_fixed_point`
- **Cap:** `frontend/src-tauri/src/audio/speaker/commands.rs:856
  enforce_max_speakers_cap`
- **Memory:** `project_diarization_root_cause.md` (root-cause state),
  `project_diarization_clustering_perf.md` (cached-matrix AHC perf, unrelated),
  `feedback_dont_infer_ground_truth_from_diarization.md` (the drift-misdiagnosis lesson)
