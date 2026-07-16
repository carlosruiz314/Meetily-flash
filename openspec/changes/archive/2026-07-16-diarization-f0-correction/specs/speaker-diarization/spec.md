> **⚠ WITHDRAWN 2026-07-10 — do not sync to canonical.** The F0 pitch corrective layer
> requirement below was disproven on real contaminated audio. Both real-data tests failed
> (cde5c264: recovered 4s of ~600s target, stole 24% of early speech; 95db: collapsed 3→2
> speakers). Root cause: per-cluster F0 profiles inherit embedding contamination — the approach is
> circular. F0 correction is gated off in production. This delta spec is retained as a historical
> record of the attempt; it MUST NOT be applied to the canonical spec. See `design.md` "⚠
> REAL-DATA VALIDATION FAILED" and `tasks.md` §0 STATUS. The canonical spec's existing "Out of
> scope — sustained speaker absorption" note correctly stands: the fix is preventive (channel
> separation), not a post-hoc label correction.

## ADDED Requirements

### Requirement: F0 pitch corrective layer recovers speakers absorbed by embedding contamination

After temporal-coherence smoothing and before per-chunk labels are coalesced into
`SpeakerSegment` objects, the system SHALL apply an F0 (fundamental frequency / pitch)
corrective layer inside `sherpa_adapter.rs::process()`. The layer SHALL be a pure computation
(deterministic, no I/O) over the per-chunk labels, the decoded audio buffer (`samples: &[f32]`
and `sample_rate: u32`, with per-chunk slices accessed via each chunk's `start_sample` /
`end_sample`), and the post-smoothing centroids. It is consistent with the
existing adapter-internal functions (`cluster_by_centroids`, `smooth_to_fixed_point`) that also
take `&[Chunk]`; it is not hexagonal-domain-pure (that awaits the deferred `hexagonal-port-traits`
refactor that would migrate `Chunk` to `domain/`).

**Motivation (established root cause):** when two remote speakers are mixed on one system channel
(pre-mixed by the meeting platform before Meetily captures the audio), the louder voice dominates
each chunk's TDNN embedding, burying the quieter speaker's voiceprint. No clustering algorithm
(centroid linkage, average linkage) recovers the quieter speaker from contaminated embeddings —
stage-trace diagnostics confirmed absorption happens at the AHC step itself. F0 (pitch) is a
separate acoustic dimension that survives mix-down: the quieter speaker's vocal-fold vibration
rate is preserved in the waveform even when the voiceprint is buried. The F0 corrective layer
uses pitch to reassign chunks the embeddings got wrong.

**F0 detection:** the system SHALL compute per-chunk F0 via autocorrelation on 4×-downsampled
audio (16 kHz → 4 kHz; F0 range 80–400 Hz; lag range 10–50 samples at 4 kHz). A 4-tap boxcar
low-pass filter SHALL be applied before decimation to suppress aliasing of speech energy above
2 kHz (the filter is weak — −3.7 dB at 2 kHz — but sufficient at negligible cost; the D13
validation used unfiltered decimation and task §4.1 re-validates with the filtered
implementation). After decimation, the chunk SHALL be DC-removed (subtract the mean) so DC bias
does not produce a spurious lag-0 peak. Non-finite (NaN or Inf) sample values SHALL be replaced
with 0.0 before the boxcar filter (the boxcar propagates NaN, which would contaminate the
DC-removal mean and zero the entire chunk). The autocorrelation SHALL be normalised (NCC =
`Σ x[i]·x[i+τ] / sqrt(Σ x[i]² · Σ x[i+τ]²)`, bounded to [-1, 1]) so that amplitude (louder vs
quieter voice) does not bias the result; if the denominator is zero (all-zero chunk or
constant-DC after removal), NCC(τ) SHALL be defined as 0.0 (not NaN). A chunk is voiced (has a
usable F0) only if `max NCC(τ) ≥ 0.30`; unvoiced chunks are skipped and their labels are left
unchanged. The **strongest** NCC peak SHALL be selected (scanning short-to-long lag with
`best_ncc` initialised to 0.0, so only positive NCC values are eligible — voiced speech produces
positive NCC at the pitch lag, while negative NCC indicates anti-correlation), applying a
per-lag penalty (`adjusted = ncc − τ × 3e-4`) that breaks near-ties toward shorter lag (higher
F0). The raw NCC is retained for the voicing gate. For single-pitch harmonic signals (real
voice) the fundamental produces an equal or stronger autocorrelation than its harmonics; for a
finite pure sine the longest exact-period-multiple lag can have marginally higher NCC than the
fundamental (e.g. 240 Hz at 4 kHz: lag 50 = 3 periods, NCC=1.0 vs lag 17, NCC≈0.992), and the
penalty overcomes this without overriding genuine octave-strength differences in real speech.
So the penalised-strongest-peak IS the true F0 — no separate octave-error guard is needed at
the `detect_f0` level.

**Per-cluster F0 profile:** for each cluster, the system SHALL compute the median F0 of its
voiced members using the post-smoothing labels (before any F0 correction). A cluster with fewer
than 3 voiced members has no reliable F0 profile and SHALL be exempt from both giving and
receiving reassignments.

**Reassignment criteria:** a voiced chunk `i` currently labelled `L_i` SHALL be reassigned to
cluster `K` iff ALL THREE of the following hold:
1. `|f0_i − median_F0(K)| < 30 Hz` (the chunk's pitch matches the target register — well inside
   the 109–122 Hz inter-register gap observed on prod data).
2. `|f0_i − median_F0(L_i)| > 50 Hz` (the chunk's pitch clearly disagrees with its assigned
   cluster).
3. `median_F0(K)` and `median_F0(L_i)` are on opposite sides of 180 Hz (the inter-register
   valley floor), so the two registers are genuinely distinct.

No register-proximity octave guard is applied. Such a guard (checking `f0_i/2` against
`median_F0(L_i)`) would block every recovery when the two registers are ~2:1 apart (common for
male/female voice pairs near 120/240 Hz — the cde5c264 case): `f0_i/2 = 120.5` always matches the
120 Hz absorber cluster. Octave errors are instead handled at `detect_f0`: the strongest-NCC-peak
selection picks the fundamental for single-pitch signals, so a true-120 Hz chunk is read as 120 Hz
and kept by conjunct 2.

**Multi-pitch overlap limitation:** when two speakers talk over each other on the mixed system
channel, the strongest autocorrelation peak corresponds to the louder speaker's pitch. Chunks
where the absorbed (quieter) speaker is NOT the dominant periodic source are not reassigned — a
recall limitation, not a precision risk (the absorber's F0 fails conjunct 2). This is unavoidable
for single-pitch autocorrelation on mixed-mono audio.

Ties (a chunk matching two candidate clusters equally, or within float rounding) SHALL be broken
deterministically by smallest cluster label, and the implementation SHALL scan candidate clusters
in sorted label order so the output is independent of `HashMap` iteration order (Rust's `HashMap`
is randomly seeded per process). The layer SHALL NOT increase the cluster count (the set of
distinct output labels is a subset of the input's).

**Centroid preservation (labels only, not centroids):** the layer SHALL NOT recompute centroids.
F0-reassigned chunks receive the target cluster's label but do NOT contribute to its stored
centroid — their embeddings are contaminated by definition (that is why the embedding-based
clustering got them wrong), so including them would corrupt the cross-meeting voice profile. The
stored centroids SHALL remain the post-smoothing values (de-contaminated), preserving the
existing "Stored centroids are post-smoothing" guarantee.

Non-finite (NaN or Inf) sample values SHALL contribute 0.0 to the autocorrelation sums, so a
degenerate chunk cannot corrupt the F0 estimate or panic the layer.

#### Scenario: Absorbed speaker is recovered on a real meeting

- **GIVEN** meeting `cde5c264` (3 speakers, 83 min) where the quieter third speaker is absorbed
  from ~min 30 onward under embedding-based clustering (late-half duration ~26s post-merge)
- **WHEN** the F0 corrective layer runs as part of `process()`
- **THEN** the absorbed speaker's late-half duration is ≥ 600s (validated: 1381s of her late
  speech has F0 in her register but was mis-assigned by the embeddings)
- **AND** her early-half duration is not reduced by more than 10% (no regression on
  correctly-assigned early speech)

#### Scenario: Clean meeting is a no-op

- **GIVEN** a meeting whose embedding-based clustering is correct (every chunk's voice matches
  its cluster)
- **WHEN** the F0 corrective layer runs
- **THEN** zero chunks are reassigned
- **AND** the output labels are identical to the input labels

#### Scenario: All-same-register meeting is a no-op

- **GIVEN** a meeting where all speakers share a pitch register (all cluster medians on the same
  side of 180 Hz)
- **WHEN** the F0 corrective layer runs
- **THEN** zero chunks are reassigned (the "opposite sides of the valley" conjunct fails for
  every candidate pair)

#### Scenario: Unvoiced chunks are never reassigned

- **GIVEN** a chunk whose NCC peak is below 0.30 (silence, breath, or noise)
- **WHEN** the F0 corrective layer runs
- **THEN** the chunk's label is left unchanged regardless of how its (non-existent) F0 would
  compare to any cluster's register

#### Scenario: Boundary-register meeting is a no-op (tolerance constants are tight)

- **GIVEN** two clusters with medians 175 Hz and 185 Hz (straddling the 180 Hz boundary) and
  chunks with F0 jitter ±15 Hz around their own cluster's median
- **WHEN** the F0 corrective layer runs
- **THEN** zero chunks are reassigned (the 30 Hz match tolerance and 50 Hz disagreement minimum
  are tight enough that near-boundary registers do not trigger false reassignments)

#### Scenario: Recovery does not overshoot and steal the absorber's chunks

- **GIVEN** meeting `cde5c264` where F0 correction recovers the absorbed speaker
- **WHEN** the F0 corrective layer runs
- **THEN** the absorbed speaker's late-half duration is ≥ 600s (recovery floor) AND ≤ 1800s
  (recovery ceiling — D13 measured 1381s recoverable; exceeding 1800s indicates the fix is
  stealing the absorber's chunks rather than recovering the absorbed speaker's)
- **AND** the absorber cluster's late-half duration is not reduced by more than 15% (symmetric
  regression guard)

#### Scenario: Cluster count never increases

- **GIVEN** a clustering output with K distinct labels
- **WHEN** the F0 corrective layer runs
- **THEN** the corrected output has at most K distinct labels
- **AND** a cluster that loses all its chunks is dropped rather than preserved as a zero-duration
  phantom

#### Scenario: Centroids are unchanged by F0 correction

- **WHEN** the F0 corrective layer runs and reassigns N chunks from cluster A to cluster B
- **THEN** the centroids stored in `speaker_embeddings` for clusters A and B are identical to
  the post-smoothing values (the reassigned chunks' contaminated embeddings did not contribute)
- **AND** cross-meeting matching operates on de-contaminated voice profiles

#### Scenario: Degenerate samples do not corrupt the F0 estimate

- **GIVEN** a chunk whose audio samples contain NaN or Inf values
- **WHEN** the F0 corrective layer computes the chunk's F0
- **THEN** the non-finite values contribute 0.0 to the autocorrelation sums
- **AND** the layer does not panic

## MODIFIED Requirements

### Requirement: Temporal-coherence smoothing prevents clustering contamination and per-chunk flicker

<!--
Sync note for the archive step: this MODIFIED block carries the FULL canonical requirement text
(algorithm, parameters, all scenarios) verbatim, with one change: the "Out of scope — sustained
speaker absorption" note's final sentences are replaced to reference the established root cause
and the F0 corrective layer (added by this change). The openspec sync replaces the entire
requirement block, so the full text must be present here to avoid destroying the algorithm spec.
This HTML comment MUST be stripped before /opsx:archive (it is process metadata, not spec
content).
-->

After global agglomerative clustering assigns per-chunk speaker labels, the system SHALL apply a temporal-coherence smoothing pass to the per-chunk labels INSIDE `sherpa_adapter.rs::process()` immediately after `cluster_by_centroids` and BEFORE per-chunk labels are coalesced into `SpeakerSegment` objects. The smoothing SHALL be a pure function of the chunk labels, chunk embeddings, chunk timestamps, and cluster centroids, with no I/O. The smoothing SHALL NOT increase the cluster count, and SHALL preserve genuine speaker turns whose acoustic shift is strong and whose duration meets the minimum-segment floor. The output SHALL be deterministic.

The smoothing pass SHALL perform neighborhood-voted re-assignment: for each chunk i with current label L_i, the system SHALL compute, for each candidate label k, a vote `score(k) = Σ_{j ∈ window(i)} cos(e_j, centroid_k) · w(i,j)` where the window spans the chunk itself (j = i) and its ±W temporal neighbors (default W = 3), `e_j` is chunk j's embedding, and the weight `w(i,j)` is `self_weight` when j = i (default 0.6) and `exp(-|i−j|)` for neighbors (peak `exp(-1) ≈ 0.368` at the nearest neighbor). The self weight (0.6) is the single strongest vote, but it is low enough that a contaminated chunk's self-fit to its (wrong) centroid is still outvoted by unanimous neighbors (whose combined weight across both sides is up to ~1.106), recovering the chunk (local contamination recovery); and it is high enough that it exceeds the neighbor weight on one side alone (~0.553), so a genuine short interjection's self-vote for its own distinct centroid anchors it against split neighbor votes on either side, and an edge-of-array interjection (neighbors on only one side) is likewise preserved. Using ONLY the chunk's own embedding (no neighbors) would reduce the vote to nearest-centroid and fix nothing; using ONLY neighbors (no self) would erase genuine short interjections between two different speakers, reintroducing the over-merging the pass exists to prevent. The system SHALL reassign the chunk's label only when the winning label's normalized score exceeds the current label's normalized score by a positive confidence margin (default 0.03), so that on a clean, high-confidence input no chunk flips (the pass is a near-no-op); the margin is set low enough to recover centroid drift up to cosine ~0.97 against the true centroid, while a clean meeting's self-differential (~0.24 at a between-speaker cosine of 0.6) is well above it, so clean input stays stable. The winner SHALL be chosen deterministically (highest score, ties broken by smallest label) so the output is independent of HashMap iteration order. The system SHALL then recompute duration-weighted centroids from the cleaned labels and iterate the re-assign/recompute cycle up to a fixed cap (default 2 iterations) so that recovered chunks refine the centroids used in the next pass.

After the iteration, the system SHALL merge a same-label run shorter than `MIN_SMOOTH_SEGMENT_SECS` (default ~10 s) into a neighbor ONLY when both adjacent runs share the same label as each other (a flicker island). The system SHALL NOT merge a short run sandwiched between two different speakers (a genuine interjection); such a run is preserved by the damped-self vote's margin gate, so the floor need not (and must not) merge it.

Non-finite (NaN or Inf) embedding values in the smoothing window SHALL contribute 0.0 to the vote, so a degenerate chunk cannot corrupt the outcome. Non-finite timestamp values SHALL exclude that chunk from the window rather than corrupting the temporal ordering or panicking.

#### Scenario: Early contamination seed is absorbed

- **GIVEN** a meeting where the t=0 chunk is assigned to a spurious cluster but its ±W temporal neighbors are consistently cluster 0
- **WHEN** temporal-coherence smoothing runs
- **THEN** the t=0 chunk is reassigned to cluster 0
- **AND** no spurious cluster persists from the contamination seed

#### Scenario: Local mis-assignment is recovered when neighbors are clean

- **GIVEN** a chunk mis-assigned to cluster B whose ±W temporal neighbors are consistently cluster C (the chunk's own voice)
- **WHEN** temporal-coherence smoothing runs
- **THEN** the chunk is recovered to cluster C
- **AND** recovery requires clean neighbors — a SUSTAINED regional mis-assignment (every neighbor also mis-assigned) is NOT recovered, because the neighborhood vote reinforces the local consensus

> **Out of scope — sustained speaker absorption over a long meeting.** The neighborhood-voted
> smoothing provably cannot recover a SUSTAINED regional mis-assignment: when every temporal
> neighbor of a chunk carries the same (wrong) label, the neighborhood vote reinforces that
> consensus rather than overturning it, so the pass leaves the region unchanged by design. This
> is a structural property of any local smoothing pass, independent of why the region was
> mis-assigned. On `meeting-cde5c264-…` one of three speakers is absorbed from minute ~30 onward
> under both the production global AHC and a sequential online-centroid-tracking prototype. A
> read-only diagnostic (`test_cde5c264_embedding_drift_diagnostic`) **ruled out** the
> embedding-drift hypothesis originally suspected — the absorbed speaker's OWN late chunks are
> cos ≈ 0.85 to her early centroid (same-speaker range), NOT the ≈ 0.22 figure cited earlier
> (which was the mean cosine of ALL late chunks to her centroid, low only because most late
> chunks belong to other speakers). The root cause is now **established**: mixed-audio chunks
> (two remote speakers summed on one system channel by the meeting platform) produce TDNN
> embeddings dominated by the louder voice, burying the quieter speaker's voiceprint; no
> clustering algorithm recovers it. The F0 pitch corrective layer attempted by the
> `diarization-f0-correction` change was **disproven on real data (2026-07-10)**: per-cluster F0
> profiles are derived from the same contaminated embedding clusters, making the approach circular
> — UserB's embedding cluster has median F0 190 Hz but her actual late speech sits at 222–308 Hz,
> so correction sends her chunks to the wrong cluster and steals her low-F0 early chunks. It also
> collapsed a known-good 3-speaker meeting (95db) to 2. F0 correction is gated off in production
> (`F0_CORRECTION_ENABLED = false`). Sustained absorption on mixed-mono recordings is an
> information-theoretic limit — no post-hoc method recovers it. The preventive fix is channel
> separation (preserve mic + system as separate tracks through the pipeline), tracked as a separate
> change. Do not re-attempt a smoothing-level, clustering-level, or per-cluster-F0 fix for
> sustained absorption.

#### Scenario: Per-chunk flicker is eliminated

- **GIVEN** a clustering output with a 40 % singleton-run rate in an acoustically stable region
- **WHEN** temporal-coherence smoothing runs
- **THEN** the singleton-run rate in that region drops below 5 %
- **AND** genuine speaker turns (strong acoustic shift, duration at or above the minimum-segment floor) are preserved

#### Scenario: Genuine turn is not over-smoothed, including short interjections

- **GIVEN** a genuine speaker change with a strong acoustic shift and duration just above `MIN_SMOOTH_SEGMENT_SECS`
- **WHEN** temporal-coherence smoothing runs
- **THEN** the turn is preserved and is not merged into the neighbor
- **AND** a short interjection (run below the floor) sandwiched between two DIFFERENT speakers is also preserved, not merged

#### Scenario: Degenerate embeddings do not corrupt the vote

- **GIVEN** a chunk whose embedding contains NaN or Inf values
- **WHEN** temporal-coherence smoothing runs
- **THEN** the degenerate embedding contributes 0.0 to the vote
- **AND** the vote outcome is determined by the finite neighbors

#### Scenario: Degenerate timestamps do not corrupt the temporal ordering

- **GIVEN** a chunk array containing a NaN or Inf timestamp (e.g., from garbled Whisper output)
- **WHEN** temporal-coherence smoothing runs
- **THEN** the degenerate-timestamp chunk is excluded from neighbor windows rather than corrupting the sort
- **AND** the smoothing does not panic

#### Scenario: Cluster count never increases

- **GIVEN** a clustering output with K clusters
- **WHEN** temporal-coherence smoothing runs
- **THEN** the smoothed output has at most K clusters
- **AND** a cluster that loses all its chunks under smoothing is dropped rather than preserved as a zero-duration phantom

#### Scenario: Stored centroids are post-smoothing

- **WHEN** diarization completes with temporal-coherence smoothing
- **THEN** the centroids stored in `speaker_embeddings` equal the recomputed post-smoothing centroids
- **AND** cross-meeting matching uses de-contaminated voice profiles

#### Scenario: Long meeting smoothing stays bounded

- **GIVEN** a meeting at the chunk cap (`MAX_DIARIZATION_CHUNKS` = 600)
- **WHEN** temporal-coherence smoothing runs with up to the iteration cap
- **THEN** the smoothing and centroid recompute complete in sub-second wall-clock time, consistent with the O(n·W·K) cost bound

#### Scenario: Clean meeting is a near-no-op

- **GIVEN** a meeting whose clustering output is already temporally coherent (well-separated speakers, no flicker, no contamination)
- **WHEN** temporal-coherence smoothing runs
- **THEN** the output labels are unchanged except for a negligible fraction of chunks
- **AND** the centroids are unchanged
- **AND** well-separated speakers (centroid cosine < 0.3) whose runs meet the minimum-segment floor are never merged
