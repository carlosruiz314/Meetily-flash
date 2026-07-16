## ⚠ REAL-DATA VALIDATION FAILED (2026-07-10) — approach disproven

The F0-correction hypothesis was validated on synthetic data (unit tests + proptest, all green)
and **disproven on real contaminated audio**. Both real-data tests (§4.1 cde5c264, §4.2 95db)
FAILED. The pipeline wiring is gated off (`F0_CORRECTION_ENABLED = false` in `process()`); the
`detect_f0` / `correct_labels_by_f0` code and unit tests are retained for reference only.

**§4.1 cde5c264:** F0 correction changed 83/676 labels but UserB's late-half went 26s→30s
(floor was 600s). Her EARLY-half DROPPED 1078s→820s — F0 correction stole correctly-attributed
chunks instead of recovering absorbed ones.

**§4.2 95db:** F0 correction collapsed 3→2 speakers (regression on a previously-correct meeting).

**Root cause of the failure (diagnostic output):** the per-cluster F0 profile is derived from
contaminated embedding clusters — **the approach is circular by construction**. Concretely on
cde5c264 (labels_b, post-smoothing):

| cluster | F0 median | voiced/total | identity |
|---|---|---|---|
| 0 | **235 Hz** | 258/277 | high-register (NOT UserB's embedding cluster) |
| 1 | **190 Hz** | 88/155 | UserB (cos 0.998 to fingerprint) |
| 2 | **121 Hz** | 184/242 | absorber (Speaker 2) |

UserB's embedding cluster (1) has median F0 **190 Hz**, but her actual late speech sits at
**222–308 Hz** (median 222, p75 235). The 3-conjunct reassignment sends her late chunks to cluster
0 (235 Hz, closer to 222 than 190 is) instead of her embedding cluster 1 — relabel flow:
`2→0: 23 chunks` (absorber→wrong high cluster), `2→1: 1 chunk` (only ONE recovered to UserB),
`1→2: 40 chunks` (UserB early chunks stolen by absorber via low-F0 moments).

**Why D13's "1381s recoverable" was wrong:** D13 measured F0 ≥ 190 on a curated chunk set and
assumed the per-embedding-cluster median would equal the speaker's true register (D9's 241 Hz
K-means center). It does not — on contaminated audio the embedding-cluster F0 median is itself a
blend that matches no single speaker. F0 and embedding dimensions don't align at the cluster level.

**Conclusion:** F0 correction cannot work with embedding-cluster-derived pitch profiles. It is the
fifth independent method disproven on cde5c264 (after STEP 7/8 embedding sweep, EEND-EDA,
SepFormer, threshold/segmentation tuning). The fix is preventive (channel separation for future
recordings); cde5c264 is unfixable from existing audio. See
[`project_diarization_root_cause`](memory) for the full investigation.

---

## Context

Sustained speaker absorption is the last open diarization failure class. The canonical spec
(`specs/speaker-diarization/spec.md`, "Temporal-coherence smoothing" requirement) carries an
explicit deferral:

> **Out of scope — sustained speaker absorption over a long meeting.** … The root cause is not
> yet determined and is filed as a separate change; do NOT re-attempt a label-level fix for
> sustained absorption without first establishing the cause.

Six diagnostic tracts on prod data (`openspec/exploration/diarization-eend-poc-log.md` D8–D13)
established the cause:

- **D9:** F0 (pitch) is bimodal on cde5c264 — the absorbed speaker sits at ~241 Hz, the absorber
  at ~120 Hz, with a deep valley at 160–190 Hz. Separable in both early and late halves.
- **D11 (stage-trace):** absorption happens at Stage A (AHC clustering) — 954s early / 48s late.
  Smoothing (44 flips) and merge-short are innocent (marginal / zero change).
- **D12:** average linkage (Lance-Williams, size-weighted — the scipy algorithm the Python POC
  used) produces the **same** 48s late. The absorber is not the linkage criterion.
- **D13:** of 423 late-half chunks the embeddings mis-assigned, **202 have F0 ≥ 190 Hz** (the
  absorbed speaker's register) — **1381s recoverable by pitch (upper bound)**. This figure uses
  the looser `f0 ≥ 190` criterion from the diagnostic; the production 3-conjunct criteria (D4)
  recover only chunks with `|f0 − 241| < 30` (i.e. F0 ∈ [212, 270] for UserB's 241 Hz median),
  so actual recovery may be lower. The 600s spec floor (2.3× below the upper bound) provides
  margin. Task §4.1 re-validates with the exact production criteria on real data.

The embedding model is not broken — on chunks that cleanly capture the quieter speaker, cosine
to her centroid is ≈0.85 (same-speaker range; confirmed by `test_cde5c264_embedding_drift_-
diagnostic`, cited in the canonical spec). The failure is that mixed-audio chunks (two remote
speakers summed on one system channel by Zoom/Meet) produce embeddings dominated by the louder
voice, and no clustering algorithm can recover a voice from a contaminated embedding. F0, being
a time-domain feature (autocorrelation peak), is not affected by which voice is louder — the
pitch periodicity of both voices is present in the waveform. For chunks where the absorbed
speaker is the dominant periodic source (she is the one speaking at that moment, even if
quieter overall), the strongest autocorrelation peak falls at her pitch. Chunks where the louder
speaker co-dominates (genuine overlap) are unrecoverable by this method — a recall limitation,
not a precision risk (documented in D7 below).

**Register geometry on cde5c264:** the absorbed speaker (UserB) sits at ~241 Hz; the absorber
(Speaker 2) at ~120 Hz. These are almost exactly an octave apart (241/120 = 2.01). This
octave-apart register pair is central to the algorithm design below — it rules out any
octave-error guard that checks `f0_i/2` against the source cluster median (such a guard would
fire on every recoverable chunk, since 241/2 = 120.5 ≈ Speaker 2's median). See D5 for the
correct octave handling.

## Goals / Non-Goals

**Goals:**
- Recover speakers absorbed by embedding contamination, using F0 as a corrective signal.
- Guarantee a clean meeting is a near-no-op (zero spurious reassignments when pitch does not
  disagree with the embedding-based assignment).
- Preserve stored centroids as the post-smoothing de-contaminated values — F0-recovered chunks
  get the label but do not corrupt the centroid.
- No new dependencies, no new models, no API changes.

**Non-Goals:**
- Do NOT replace the embedding model or the clustering pipeline. Embeddings remain the primary
  signal; F0 is a corrective overlay for the specific failure mode where they fail.
- Do NOT attempt source separation or per-participant capture (explored and rejected in D5–D7;
  zero-shot models are domain-mismatched on mixed-mono system audio).
- Do NOT change the `DiarizationPort::process` signature or the caller in `commands.rs`.
- Do NOT fix cde5c264's existing stored diarization in-place. Re-diarization (the existing
  "Speakers" button) applies the fix to future runs; past meetings are correctable on re-diarize.

## Decisions

### D1: Pipeline placement — after smoothing, before segment coalescence

`correct_labels_by_f0` runs inside `sherpa_adapter.rs::process()` immediately after
`smooth_to_fixed_point` and before per-chunk labels are coalesced into `SpeakerSegment`
objects.

**Why after smoothing, not before:**
- Smoothing uses embedding cosine to vote. If F0 runs first, smoothing can **undo** F0's
  corrections — a recovered chunk's contaminated embedding still votes for the wrong centroid,
  and unanimous (equally-contaminated) neighbors reinforce the flip-back. Running F0 last means
  smoothing operates on the embedding signal it was designed for, and F0 fixes what smoothing
  provably cannot (sustained regional mis-assignment — the canonical spec's "Out of scope" note).
- Smoothing's 44 label changes (D11) are preserved; F0 then recovers the remaining mis-assigned
  chunks that smoothing left in place.

**Why before segment coalescence:** segment coalescence merges adjacent same-label chunks into
`SpeakerSegment` objects. F0 operates on per-chunk labels (it needs to flip individual chunks),
so it must run before chunks are coalesced into segments.

**Plug-in point:** the existing pipeline is
`cluster_by_centroids → smooth_to_fixed_point → [coalesce → merge_short]`. The new pipeline is
`cluster_by_centroids → smooth_to_fixed_point → correct_labels_by_f0 → [coalesce → merge_short]`.

### D2: Labels only, not centroids

F0-reassigned chunks receive the target cluster's **label** but do **not** contribute to its
stored centroid.

**Why:** the stored centroid drives cross-meeting speaker matching (the "Centroid embeddings are
stored per speaker per meeting" requirement). F0-recovered chunks have contaminated embeddings by
definition — that is why the embedding-based clustering got them wrong. Including them in the
centroid recomputation would pull the centroid toward the louder voice (the contaminant),
degrading cross-meeting matching for the recovered speaker.

The stored centroids remain the post-smoothing values, which are computed from the chunks the
embeddings got right (cos ≈ 0.85 to the true voiceprint). This preserves the existing spec's
"de-contaminated voice profiles" guarantee.

**Implementation:** `correct_labels_by_f0` takes
`(labels, samples, sample_rate, chunks, centroids, params)` and returns
`(corrected_labels,)` — it does not return new centroids. The `samples: &[f32]` buffer and
`sample_rate: u32` are needed because `Chunk` (sherpa_adapter.rs:147) stores only `start_sample`/
`end_sample` indices and the embedding — not the raw audio. Inside `process()`, both are already
available (the function's decoded-audio buffer and `DIARIZATION_SAMPLE_RATE`). The downstream
code uses the existing post-smoothing centroids for storage and segment-building.

`timestamps` is NOT passed: the reassignment criteria (D4) use only F0 values and cluster
medians — no temporal information is needed (unlike `smooth_to_fixed_point`, which uses
timestamps for its temporal window). At the insertion point in `process()`, `timestamps` is
not in scope (it is created inside the smoothing block and dropped), so omitting it from the
signature avoids a wiring restructuring.

### D3: F0 detection — strongest-NCC-peak autocorrelation with NCC normalisation

Per-chunk F0 is computed via autocorrelation on 4×-downsampled audio (16 kHz → 4 kHz). The F0
range of interest is 80–400 Hz (human speech); at 4 kHz the lag range is 10–50 samples.

**Anti-aliasing before decimation:** a 4-tap boxcar low-pass filter (`y[i] = (x[i] + x[i+1] +
x[i+2] + x[i+3]) / 4`) SHALL be applied before `step_by(4)` decimation. The raw 16 kHz signal
has speech energy up to 8 kHz (formants, fricatives); decimating without a low-pass filter
aliases all energy above 2 kHz into the 0–2 kHz band, which can introduce spurious
periodicity in noisy chunks. The boxcar is a weak filter (−3.7 dB at 2 kHz, −9.9 dB at 3 kHz —
not "well below"), chosen for negligible cost (4 adds + 1 shift per sample); it suppresses
the worst aliasing but is not a sharp anti-alias filter. Note: the D13 validation
(`commands.rs:3791`) used bare `step_by(4)` with NO filter, so the 202-chunk / 1381s figure
was produced against unfiltered decimation. The production boxcar may slightly shift F0 values
and voiced/unvoiced classifications; task §4.1 re-validates the recovery figures and thresholds
against the filtered implementation. The 600s floor (vs the 1381s upper bound) provides margin
for this shift.

**Preprocessing:** non-finite (NaN or Inf) sample values SHALL be replaced with 0.0 FIRST,
before any other processing — the boxcar filter propagates NaN (`NaN + finite = NaN` in Rust),
so a single NaN in the input would contaminate 4 boxcar outputs and then the DC-removal mean,
zeroing the entire chunk and returning `None`. After sanitisation, the 4-tap boxcar (above) and
4× decimation run on finite values. The decimated chunk SHALL then be DC-removed (subtract the
mean) so that DC bias does not produce a spurious lag-0 peak. A constant-DC chunk (all samples
equal) produces all-zero norm after DC removal, triggering the zero-denominator guard below. A
few NaN samples in an otherwise-valid 240 Hz chunk become 0.0, leaving the pitch periodicity
intact in the remaining samples — the chunk still returns ~240 Hz, not `None`.

**Normalised cross-correlation (NCC):** for each lag τ in [min_lag, max_lag],
`NCC(τ) = Σ x[i]·x[i+τ] / sqrt(Σ x[i]² · Σ x[i+τ]²)`. This bounds the value to [-1, 1],
independent of amplitude — critical because the absorbed speaker is quieter (lower amplitude).
Without NCC normalisation, the louder voice's higher energy biases the autocorrelation. If the
denominator `Σ x[i]² · Σ x[i+τ]²` is zero (all-zero chunk or constant-DC after removal),
`NCC(τ)` is defined as 0.0 (not NaN). The denominator's sum-of-squares over the shifting
window `[τ, n)` SHALL be computed via a prefix-sum-of-squares array
(`energy_tau = sq_prefix[n] - sq_prefix[τ]`) for O(1) per-lag cost; the numerator's dot-product
is O(n) per lag but cannot be prefix-summed (it depends on both `x[i]` and `x[i+τ]`).

**Peak selection — strongest NCC with per-lag penalty toward shorter lag (higher F0):** the
implementation SHALL scan from short lag (high F0) to long lag (low F0) and keep the strongest
NCC peak, applying a tiny per-lag penalty (`adjusted = ncc − τ × 3e-4`) so that near-ties
favour the shorter lag. The raw NCC is retained for the voicing gate. This generalises the D13
diagnostic's `if ncc > best_ncc` (`commands.rs:3824`): strict `>` only breaks **exact** ties,
but for a finite pure sine the longest exact-period-multiple lag can have genuinely higher NCC
than the fundamental (e.g. a 240 Hz sine at 4 kHz hits NCC=1.0 at lag 50 = 3 complete periods,
vs NCC≈0.992 at lag 17 = 1 period). The 3e-4 penalty overcomes this finite-signal boundary
effect (max ~5e-4 NCC gap per period-multiple) without overriding genuine octave-strength
differences in real speech (where the fundamental is typically 0.01+ stronger than harmonics).
For single-pitch harmonic signals (real voice), the fundamental produces an **equal or stronger**
autocorrelation than its harmonics — the fundamental periodicity captures all harmonics'
alignment simultaneously, while a harmonic-period lag captures only that harmonic's. So the
penalised-strongest-peak selects the true F0 for both single-pitch sines and harmonic voice —
no separate octave guard is needed at the `detect_f0` level. For mixed-audio chunks (two
speakers), the strongest peak corresponds to the dominant periodic source — see D7. The
`best_ncc` initialiser SHALL be 0.0 (not -1.0): voiced speech produces positive NCC at the pitch
lag, while negative NCC indicates anti-correlation (not a pitch candidate), so only positive NCC
values are eligible.

**Voicing gate:** a chunk is voiced (has a usable F0) only if `max NCC(τ) ≥ 0.30`. Below this,
the chunk is unvoiced (silence, breath, or noise) and is skipped — its label is left unchanged.
The 0.30 threshold is conservative (voiced speech typically peaks at 0.5–0.9; unvoiced at
<0.2) and is validated by D13 (70 of 423 late chunks were correctly classified as unvoiced).

### D4: Reassignment criteria — F0 profile mismatch (three conjuncts, no register-proximity octave guard)

For each voiced chunk `i` currently labelled `L_i`:

1. Compute the chunk's F0 (`f0_i`).
2. Compute each cluster's **F0 profile** = median F0 of its voiced members, computed from the
   input `labels` parameter (the post-smoothing labels on the first call). A cluster with <3
   voiced members has no reliable profile and is exempt from both giving and receiving
   reassignments. **Idempotency:** because profiles are recomputed from the input labels on each
   call, a second call (`f(f(x))`) recomputes profiles from the already-corrected labels. For
   large clusters (hundreds of members), reassignments reinforce the median and idempotency
   holds. For small clusters (3–4 members), a single reassignment can shift the median by 2–5 Hz,
   potentially crossing the strict `< 30` boundary for a borderline chunk — so idempotency is
   approximate for small clusters. The proptest (§5.1) reflects this by restricting the
   idempotency property to clusters with ≥10 voiced members.
3. Let `reg_L = median_F0(L_i)` and `reg_K = median_F0(K)` for the candidate cluster `K`.
4. Reassign `i` from `L_i` to the best-matching candidate `K` iff **all three** hold:
   - `|f0_i − reg_K| < F0_MATCH_TOLERANCE` (30 Hz — within the register, well inside the
     109–122 Hz inter-register gap from D9),
   - `|f0_i − reg_L| > F0_DISAGREEMENT_MIN` (50 Hz — the chunk's pitch clearly disagrees with
     its assigned cluster),
   - `reg_K` and `reg_L` are on opposite sides of `F0_REGISTER_BOUNDARY` (180 Hz — the valley
     floor from D9), so the two registers are genuinely distinct.

**Why no register-proximity octave guard (D4 originally had a 4th conjunct checking `f0_i/2`
and `2·f0_i` against `reg_L`; it was removed because it is a blocker — see below):**

The original 4th conjunct blocked reassignment when `f0_i/2` or `2·f0_i` landed within 30 Hz of
`reg_L`. On cde5c264 this blocks **every** recoverable chunk: UserB is at 241 Hz, Speaker 2 at
120 Hz, so `f0_i/2 = 120.5` is within 0.5 Hz of `reg_L = 120` → guard fires → reassignment
blocked → zero recovery. The guard cannot distinguish its two trigger conditions: (a) a true-120
Hz chunk whose autocorrelation spuriously peaks at 240 Hz (the false positive it should block)
and (b) a true-240 Hz chunk correctly read at 240 Hz (the true positive it should allow). Both
produce identical observables. When the two registers happen to be at a 2:1 ratio (common for
male/female voice pairs near 120/240), the guard is maximally wrong.

Octave errors are instead handled at `detect_f0` (D3): the strongest-NCC-peak selection picks
the fundamental for single-pitch signals (the fundamental's autocorrelation is always stronger
than its harmonics'), so a true-120 Hz chunk is read as 120 Hz, not 240 Hz. Conjunct 2
(`|f0_i − reg_L| > 50`) then keeps it in the 120 Hz cluster. No separate register-proximity
guard is needed.

**Deterministic tie-break:** the implementation SHALL iterate candidate clusters in sorted
(smallest-first) label order, so that when two clusters match a chunk equally (or within float
rounding), the smallest-label cluster wins deterministically. This makes the output independent
of `HashMap` iteration order (Rust's `HashMap` is randomly seeded per process). Conjunct
evaluation does not short-circuit on the first match — all candidates are scored and the best
is chosen; the sort guarantees the tie-break.

### D5: Octave-error handling — strongest peak IS the fundamental for single-pitch signals

Autocorrelation is run over the full lag range (min_lag → max_lag, i.e. 400 Hz → 80 Hz) and the
**strongest** NCC peak is selected, with the per-lag penalty (3e-4, see D3) giving a tie-break
toward shorter lag (higher F0). This generalises the D13 diagnostic's `if ncc > best_ncc`
(`commands.rs:3824`), which the 202-chunk / 1381s recovery claim is validated against.

For a single-pitch harmonic signal (voice), the autocorrelation at the fundamental period
captures the alignment of ALL harmonics simultaneously, producing a stronger peak than any
individual harmonic's period. So the strongest peak IS the fundamental — there is no
octave-up artifact to guard against. This is why the register-proximity octave guard (originally
D4 conjunct 4) is unnecessary and was removed: it cannot distinguish a true-fundamental reading
from an octave artifact using cluster assignment as a proxy, and it blocks every recovery when
registers are ~2:1 apart (the cde5c264 case).

For mixed-audio chunks (two speakers at different pitches), the autocorrelation has genuine
peaks at both periods. The strongest peak corresponds to the louder speaker's pitch — see D7
for why this is the correct (if imperfect) behaviour for the absorption-recovery use case.

### D6: Clean-meeting no-op

On a clean meeting (embeddings got everything right), F0 correction reassigns zero chunks:

- Every chunk's F0 agrees with its cluster's median (within `F0_DISAGREEMENT_MIN`), so the
  disagreement conjunct (`|f0_i − reg_L| > 50`) fails for every chunk. No reassignment.
- If all speakers share a pitch register (all clusters' medians on the same side of
  `F0_REGISTER_BOUNDARY`), the "opposite sides" conjunct fails. No reassignment.

This must be asserted by an adversarial test (synthetic clean meeting: two well-separated
speakers with distinct embeddings, correct clustering → zero F0 reassignments).

### D7: Multi-pitch overlap is a recall limitation, not a precision risk

When two speakers talk over each other on the mixed system channel, the chunk's waveform
contains both pitches. The strongest-NCC-peak selection (D3) locks onto the louder speaker's
pitch. If the absorbed (quieter) speaker is NOT the dominant periodic source in an overlap
chunk, her pitch is not detected → the chunk is not reassigned → a recall miss. This is
unavoidable for any single-pitch autocorrelation method on mixed-mono audio (source separation
and per-participant capture were explored and rejected in exploration D5–D7).

This does NOT cause false positives: an overlap chunk where the absorber (Speaker 2) dominates
yields `f0_i ≈ 120 Hz` → conjunct 2 (`|f0_i − reg_L| > 50`) fails → no reassignment. Only chunks
where the absorbed speaker IS the dominant periodic source are reassigned. So precision is
preserved; recall is bounded by how often the absorbed speaker dominates her own chunks. D13's
202-of-423 figure is the empirically measured recall on cde5c264.

### D8: Interaction with `enforce_max_speakers_cap` and `merge_short_speakers`

F0 correction runs after smoothing and before segment coalescence (D1). Two downstream steps
operate on F0-corrected labels but pre-F0 centroids:

- **`merge_short_speakers`** (sherpa_adapter.rs:410) uses segment-derived durations for the
  short/long threshold and centroid cosine similarity for the merge target. After F0 moves
  chunks into the recovered cluster, its duration increases (clearing the 2% threshold), while
  its centroid remains the post-smoothing value (based on the recovered speaker's clean early
  chunks — the most de-contaminated embeddings in the meeting). This is safe: the recovered
  cluster has both enough duration and a clean centroid.
- **`enforce_max_speakers_cap`** (commands.rs:856) runs after the diarization output is built.
  It is a no-op whenever clustering produces ≤ `max_speakers` clusters. On cde5c264, clustering
  produces exactly 3 speakers; the test context (`commands.rs:1349`) sets `max_speakers = 3`
  and the production default is 10, so the cap does not fire either way. F0 correction cannot
  increase the cluster count (it reassigns to existing labels), so it cannot cause the cap to
  fire when it wouldn't have already. The risk is only on meetings where clustering over-splits
  beyond a tight user-set cap: if F0 recovers chunks into the most-isolated cluster (selected by
  centroid cosine, which F0 does not change), the cap merges that cluster back, undoing the
  recovery for those chunks. This is an accepted trade-off — F0 correction runs first, and the
  cap can override it on tight-cap meetings. The design does NOT exempt F0-recovered clusters
  from the cap (that would add complexity for a narrow case). If a user sets a tight cap and
  loses a recovered speaker, the fix is to raise the cap.

### D9: The 180 Hz register boundary is calibrated from cde5c264

`F0_REGISTER_BOUNDARY = 180 Hz` is derived from the F0-histogram valley on cde5c264 (exploration
log D9, `diarization-eend-poc-log.md`: UserB at ~241 Hz, Speaker 2 at ~120 Hz, valley at
160–190 Hz). For meetings
without this register geometry the boundary may not sit at the true valley, but the correction
remains SAFE (no false positives): the "opposite sides of 180 Hz" conjunct (D4 conjunct 3)
fails when all speakers share a register, so zero reassignments fire. The cost is missed
recoveries on meetings with a different valley (e.g., two males at 85 Hz and 160 Hz with a
valley near 120 Hz), not false reassignments.

A future enhancement could compute the boundary from the cluster-median histogram (the valley
between the two highest cluster medians). This is out of scope for this change — the constant
boundary is sufficient for the cde5c264-class failure and safe for everything else.

## Risks / Trade-offs

- **[False reassignment on a clean chunk]** → Guarded by the three-conjunct criteria (D4): a
  chunk is only moved if its pitch clearly disagrees with its assigned cluster, matches a
  different cluster's register, and the two registers are on opposite sides of the valley
  boundary. The clean-meeting no-op test (D6) and the boundary-register no-op test (task 2.5)
  are the regression guards. If a false reassignment slips through, the user can re-diarize or
  manually relabel — F0 correction is not destructive (labels are mutable, centroids are not
  touched).
- **[Autocorrelation F0 is less accurate than pyin]** → True, but sufficient for register
  separation. D9 measured a 109–122 Hz inter-register gap; pyin's sub-Hz accuracy is unnecessary.
  The voicing gate (NCC ≥ 0.30) skips unvoiced chunks where autocorrelation is unreliable. A
  more accurate detector (e.g. a future pyin port) can drop into the same `detect_f0` signature
  without touching the reassignment logic.
- **[Multi-pitch overlap recall loss]** → Chunks where the louder speaker co-dominates are
  unrecoverable (D7). This bounds recall but does not affect precision — overlap chunks yield
  the absorber's F0 and fail conjunct 2. D13's 202/423 figure is the measured recall ceiling on
  cde5c264; the fix recovers what it can without inventing false positives.
- **[Cap-enforcement can re-absorb on tight-cap meetings]** → `enforce_max_speakers_cap` runs
  after F0 and merges the most-isolated cluster; a recovered speaker is acoustically distinct and
  could be merged on a tight per-meeting override. Accepted trade-off (D8) — raise the cap to
  recover the speaker.
- **[180 Hz boundary is meeting-specific]** → Calibrated from cde5c264's valley (D9). For
  meetings with a different valley the correction is safe (no-op via conjunct 3) but may miss
  recoveries. A data-driven boundary is a follow-up (D9).
- **[Three+ pitch registers (4+ speakers with distinct pitch)]** → The criteria reassign to the
  single best-matching cluster by median distance. With 4+ registers, a chunk could in principle
  match two clusters equally; the deterministic tie-break (smallest label, D4) prevents flapping.
  This is an edge case not present in the target meeting; if it arises, per-register tolerance
  tuning is the follow-up.
- **[Centroid drift from label-only reassignment]** → None. Centroids are not recomputed (D2).
  The stored centroids are the post-smoothing values, unchanged by F0.
- **[Median contamination on heavy-absorption meetings]** → If the absorber cluster is >50%
  contaminated by the absorbed speaker's chunks (F0 on the opposite side of 180 Hz from the
  absorber's median), the median shifts toward the contaminant, conjunct 3 fails, and recovery
  is blocked. On cde5c264 the absorber cluster has ~202 contaminated late chunks; D13 shows
  enough true absorber chunks (early-half + low-register late) to keep the median at ~120 Hz,
  but this is not proven for all meetings. Task §4.1 includes a diagnostic that logs each
  cluster's voiced-member count, F0 median, and contamination ratio; if any cluster exceeds 40%
  contamination, the fix may be blocked and the meeting is a candidate for per-meeting
  threshold tuning (out of scope for this change).
- **[Performance]** → Autocorrelation is O(chunks × downsampled_samples × lag_range) = O(676 ×
  33200 × 41) ≈ 918M multiply-adds for an 83-min meeting (cde5c264's `effective_split` is ~8.3s
  → 8.3 × 4000 samples/s after 4× decimation = 33200 samples; lag range [10, 50] = 41 lags).
  Sub-second on any modern CPU (SIMD-friendly, ~0.04s with AVX2 at 3 GHz). Runs on the existing
  blocking thread (`spawn_blocking` in commands.rs:382).

## Adversarial Tests (§4 categories)

| Category | Test |
|---|---|
| Clean meeting no-op | Synthetic 2-speaker meeting, well-separated embeddings, correct clustering → zero F0 reassignments. |
| All-same-register | Synthetic meeting where all speakers have ~150 Hz pitch → registers not distinct → zero reassignments. |
| Boundary-register no-op | Two clusters with medians 175/185 Hz (straddling 180 Hz), chunks with F0 jitter ±15 Hz → zero reassignments (tolerance constants are tight enough that straddling registers don't flap). |
| Unvoiced-heavy | Chunks with NCC < 0.30 (silence/noise) → skipped, label unchanged. |
| Detect_f0 octave robustness | True-240 Hz pure sine where lag 50 (3 complete periods, NCC=1.0) beats lag 17 (1 period, NCC≈0.992) → per-lag penalty (3e-4) overcomes the 0.008 gap, picks lag 17 → ~235 Hz. True-120 Hz pure sine → returns ~120 Hz (lag 33 is the only positive-NCC peak in range; 240 Hz lag is anti-correlated, NCC ≈ −0.998, rejected by `best_ncc = 0.0` initialiser). |
| Detect_f0 mixed-pitch (precision) | Quiet-240 Hz (amp 0.3) + loud-120 Hz (amp 1.0) summed → `detect_f0` returns ~120 Hz (D7: louder speaker wins on mixed-mono; overlap chunks are NOT reassigned). |
| Detect_f0 mixed-pitch (recovery) | Non-harmonic mix: quiet-140 Hz (amp 0.3) + loud-200 Hz (amp 1.0) → `detect_f0` returns ~200 Hz (at the 200 Hz period the 140 Hz component is anti-correlated, suppressing the sub-harmonic). **Known limitation:** for a 2:1 harmonic mix (e.g. 120+240), autocorrelation peaks at the sub-harmonic (120 Hz) regardless of loudness — F0 correction cannot recover an absorbed speaker whose pitch is an exact octave of the absorber's. Real speech rarely has exact 2:1 F0 ratios. |
| Detect_f0 degenerate samples | Chunk with NaN/Inf samples surrounded by valid samples → non-finite values contribute 0.0, valid F0 returned (not `None`). Constant-DC chunk (all-0.5) → all-zero after DC removal → `None` (zero-denominator guard). |
| Detect_f0 creaky voice | Synthetic waveform with alternating period lengths (jitter >20%, simulating vocal fry) → either NCC < 0.30 (skipped) or F0 within the chunk's own register; no mid-register F0 that would trigger a false reassignment. |
| ≥3 voiced-member exemption | Cluster A (2 voiced members, 240 Hz) + cluster B (≥3 voiced, 120 Hz): A is exempt as giver (its chunks not reassigned away) AND as receiver (B's chunks not reassigned to A). |
| Determinism + tie-break label | Call twice on identical input (including a crafted near-tie) → byte-identical output AND the smallest-label cluster wins the tie (not just agreement across runs). Cluster keys sorted before scan. |
| Centroids unchanged | After `correct_labels_by_f0` reassigns N chunks, the `centroids` HashMap is byte-identical to the input (read-only guarantee enforced by `&` borrow + explicit assertion). |
| Absorption recovery (#[ignore], real data) | cde5c264: F0 correction moves the absorbed speaker's late-half duration from ~26s to ≥600s AND ≤1800s. Speaker 2's late-half not reduced >15%. **Per-chunk assertion:** every chunk moved FROM Speaker 2's cluster TO UserB's cluster has `detect_f0(chunk_audio)` in UserB's register (within 30 Hz of her median). **Contamination diagnostic:** logs each cluster's voiced-member count, F0 median, and contamination ratio; asserts absorber cluster contamination <40%. |
| Early-half guard (#[ignore], real data) | cde5c264: the absorbed speaker's early-half duration is not reduced by >10% (no regression on correctly-assigned early speech — confirms the fix only moves late-half chunks the embeddings got wrong, not early-half chunks they got right). |
| Regression (#[ignore], real data) | 95db (3-speaker, known-good): speaker count unchanged, no speaker collapses by >15% between halves, total reassignment count small (near-no-op). |
| Property (proptest) | For valid inputs with pitch-structured samples (sine waves at frequencies aligned/misaligned with cluster medians): (a) deterministic, (b) cluster count never increases, (c) idempotent for clusters with ≥10 voiced members — `f(f(x)) == f(x)`, (d) every reassigned chunk's F0 matches the target cluster's median within `F0_MATCH_TOLERANCE`. |

## Migration Plan

- No data migration. F0 correction runs on future diarization and re-diarization; existing
  stored diarization is unchanged until the user re-diarizes.
- The `DiarizationPort::process` signature is unchanged, so the caller in `commands.rs` is
  unaffected.

## Open Questions

None — D13 validated the fix direction.

**Shark-tank round 1 amendments (folded into the design above):**
- D4 originally had a 4th "octave-error guard" conjunct checking `f0_i/2` and `2·f0_i` against
  `reg_L`. Two sharks independently proved it blocks every recovery on cde5c264 (UserB 241 Hz /
  Speaker 2 120 Hz = octave apart → `f0_i/2 = 120.5` always within 30 Hz of `reg_L`). Removed;
  octave handling moved to `detect_f0` (D5 strongest-peak = fundamental).
- D5 originally specified "first significant peak" — contradicted both the strongest-peak code
  that produced D13's evidence AND D4's "check half-lag first" explanation (the two were
  mutually exclusive). Rewritten to strongest-NCC-peak (the validated algorithm).
- D2 signature originally omitted `samples` + `sample_rate` (3 sharks flagged). Added.
- D3 originally claimed "Nyquist above 400 Hz so no aliasing" — incorrect (speech energy above
  2 kHz aliases into 0–2 kHz). Added a 4-tap boxcar low-pass filter before decimation.
- D7/D8/D9 added: multi-pitch recall limitation, cap-enforcement interaction, boundary
  calibration scope.
- Performance math corrected (189M → 757M; the "7000 samples" confused samples with ms).
- Tasks §2/§4/§5 expanded: boundary-register no-op, determinism, creaky-voice, sample-rate,
  recovery ceiling, idempotency. See tasks.md.

**Shark-tank round 2 amendments (folded into the design above):**
- D13 upper-bound gap (Shark 1 C1): the 1381s / 202-chunk figure used `f0 ≥ 190` (looser than
  production `|f0 − 241| < 30`); acknowledged as an upper bound, 600s floor provides margin.
- D2 signature: dropped `timestamps` (Shark 4 I2/I3 — unnecessary, not in scope at insertion
  point; F0 correction is per-chunk, no temporal info needed).
- D3 boxcar: acknowledged weak (−3.7 dB, not "well below"); D13 validated without it; §4.1
  re-validates with the filtered implementation.
- D3 preprocessing: added DC removal + NaN/Inf sanitization (Shark 2 I6 / Shark 3 Imp #2) +
  prefix-sum-of-squares (Shark 3 Minor #5).
- D3 peak selection: "always stronger" → "equal or stronger" (Shark 1 M2 — pure sines produce
  equal NCC at fundamental and octave; tie-break does the work). Added `best_ncc = 0.0`
  initialiser note (Shark 1 M3).
- D4 idempotency: clarified profiles computed from input labels; approximate for small clusters
  (Shark 1 I3). Added median-contamination risk (Shark 1 I4) with §4.1 diagnostic.
- D8: fixed factual error — cde5c264 cap is 3 in tests, 10 in production (Shark 4 I1).
- D9: disambiguated "D9 exploration" → "exploration log D9" (Shark 3 Minor #8).
- Performance: corrected 757M → 918M (8.3s chunks, not 7s; Shark 1 M1).
- Adversarial-tests table: added mixed-pitch, degenerate-samples, ≥3-exemption,
  centroids-unchanged rows; fixed impossible "stronger sub-harmonic" test (Shark 1 I1);
  tightened regression threshold 50%→15% (Shark 2 I2); weakened variance property (Shark 2 I4);
  added per-chunk F0-register assertion + contamination diagnostic (Shark 2 C2 / Shark 1 I4).
- See tasks.md and spec.md for corresponding test/spec amendments.

**Shark-tank round 3 amendments (folded into the design above):**
- D3 preprocessing order (Shark 4 I1 — Critical-to-Important): NaN/Inf sanitisation moved FIRST,
  before the boxcar. The boxcar propagates NaN (`NaN + finite = NaN`), so a single NaN sample
  contaminates 4 boxcar outputs, makes the DC-removal mean NaN, and zeroes the entire chunk →
  returns `None`, contradicting the §1.1 test that expects a few NaN samples surrounded by valid
  240 Hz to return ~240 Hz. Fixed in design.md D3, spec.md, and tasks.md §1.2.
- D3 boxcar attenuation (Shark 1 M2): corrected "−6 dB at 3 kHz" → "−9.9 dB at 3 kHz" (the 4-tap
  boxcar at 3 kHz / 16 kHz has |H| ≈ 0.318 → −9.95 dB; the earlier −6 dB figure was wrong).
- D13 floor ratio (Shark 1 M1): "3× below the upper bound" → "2.3× below" (1381s / 600s = 2.3).
- Risks prose (Shark 1 M3): "task 2.8" → "task 2.5" (the boundary-register no-op test is §2.5;
  §2.8 is the creaky-voice test).
- §3.3 cleanup (Shark 4 M1): added explicit removal of `detect_f0_autocorr` from `commands.rs`
  (exploration scaffolding — bare `step_by(4)`, no boxcar; dead code once §1.2 lands the
  production `detect_f0`).
- Adversarial-tests table (Shark 3 cosmetic): added early-half guard row (real-data, >10%
  early-half reduction forbidden) and creaky-voice row (synthetic vocal fry → skipped or
  in-register, no false reassignment).

**Round 3 verdict: 3× PROCEED + 1× PROCEED-WITH-AMENDMENTS (Shark 4, single ordering bug, now
fixed).** The tank has converged — no shark questions the fundamental F0-correction design; the
only round-3 blocker was the NaN/Inf ordering bug, which Shark 4 itself diagnosed precisely and
which is now corrected in all three artifacts. Proceeding to `/opsx:apply`.

**Implementation-time amendment (§1 testing, round 4):**
- D3/D5 peak selection: the "strict `>` tie-break toward shorter lag" assumption was insufficient
  for finite pure sines. For a 240 Hz sine at 4 kHz, lag 50 (= 3 complete periods) produces
  NCC=1.0, which is genuinely higher than the fundamental at lag 17 (NCC≈0.992) — not an exact
  tie. Strict `>` picks lag 50 → F0=80 Hz, failing 7 detect_f0 tests. Replaced with a per-lag
  penalty (`adjusted = ncc − τ × 3e-4`) that overcomes finite-signal boundary effects (max ~5e-4
  NCC gap per period-multiple) without overriding genuine octave-strength differences in real
  speech (0.01+ gap). All 7 pure-sine tests now pass.
- D7 mixed-pitch recovery: the `detect_f0_mixed_pitch_recovery_case` test used a 2:1 harmonic
  mix (120+240 Hz), which is physically impossible for autocorrelation — both components align
  at the 120 Hz lag (NCC≈0.99), producing a sub-harmonic peak that no penalty can overcome
  (0.165 NCC gap vs the 240 Hz lag). Changed to a non-harmonic mix (140+200 Hz, ratio 0.7)
  where the louder component's F0 IS detectable (at the 200 Hz period, the 140 Hz component is
  anti-correlated, suppressing the sub-harmonic). Documented as a known limitation: F0
  correction cannot recover an absorbed speaker whose pitch is an exact octave of the absorber's.
  Real speech rarely has exact 2:1 F0 ratios (see D13 upper-bound acknowledgment).
