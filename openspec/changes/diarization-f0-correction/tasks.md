## 0. STATUS — REAL-DATA VALIDATION FAILED (2026-07-10)

F0 correction passes all synthetic tests (§1, §2, §3.1, §5.1) but **FAILED both real-data tests
(§4.1 cde5c264, §4.2 95db)**. The approach is fundamentally circular — per-cluster F0 profiles are
derived from contaminated embedding clusters. See `design.md` "⚠ REAL-DATA VALIDATION FAILED"
section for the full diagnostic. The pipeline wiring (§3.2) is gated off behind
`F0_CORRECTION_ENABLED = false`; `detect_f0` / `correct_labels_by_f0` + unit tests retained for
reference. §6 (archive) is BLOCKED — this change does not achieve its goal and should not be
archived as successful. Awaiting user decision: abandon, or pivot to channel separation (the only
non-circular fix per the root-cause investigation).

## 1. F0 detection (pure helper, test-first)

- [x] 1.1 Write failing tests in `sherpa_adapter.rs` for `detect_f0(samples, sample_rate) ->
      Option<f32>`:
      - 240 Hz sine at 16 kHz → ~240 Hz.
      - 120 Hz sine at 16 kHz → ~120 Hz.
      - White noise → `None` (NCC < 0.30).
      - True-240 Hz pure sine where lag 50 (3 complete periods, NCC=1.0) beats lag 17 (1 period,
        NCC≈0.992) → ~235 Hz (per-lag penalty 3e-4 overcomes the 0.008 gap, D3/D5).
      - True-120 Hz pure sine → ~120 Hz (the 240 Hz lag is anti-correlated, NCC ≈ −0.998,
        rejected by `best_ncc = 0.0` initialiser).
      - Mixed-pitch (precision): quiet-240 Hz (amp 0.3) + loud-120 Hz (amp 1.0) summed → ~120 Hz
        (D7: louder speaker wins on mixed-mono; overlap chunks yield the absorber's F0 and fail
        conjunct 2 — no false reassignment).
      - Mixed-pitch (recovery): non-harmonic mix quiet-140 Hz (amp 0.3) + loud-200 Hz (amp 1.0)
        → ~200 Hz (at the 200 Hz period the 140 Hz component is anti-correlated, suppressing the
        sub-harmonic). KNOWN LIMITATION: a 2:1 harmonic mix (e.g. 120+240) is impossible for
        autocorrelation — both components align at the 120 Hz lag; do not construct this case.
      - 240 Hz sine clipped to ±0.8 → F0 within ±10 Hz of 240.
      - All-zero samples → `None` (NaN-denominator guard: NCC defined as 0.0 when energy is 0).
      - Constant-DC chunk (all-0.5 samples) → `None` (all-zero after DC removal → zero-denominator
        guard fires identically).
      - Chunk with a few NaN/Inf samples surrounded by valid 240 Hz samples → ~240 Hz (non-finite
        values replaced with 0.0 before entering the autocorrelation sums; valid F0 returned, not
        `None`).
      - 200-sample (50 ms at 4 kHz post-decimation) 240 Hz sine → ~240 Hz or `None`, never a
        garbage value like 480 Hz.
- [x] 1.2 Implement `detect_f0`: replace NaN/Inf samples with 0.0 (BEFORE the boxcar — the
      boxcar propagates NaN, which would contaminate the DC-removal mean and zero the chunk) →
      4-tap boxcar low-pass → 4× decimate → DC remove (subtract mean) → prefix-sum-of-squares →
      NCC scan over lag range [ds_rate/400, ds_rate/80] with `best_ncc` initialised to 0.0 (only
      positive NCC eligible) → strongest NCC peak with per-lag penalty (`adjusted = ncc − τ ×
      3e-4`, raw NCC retained for voicing gate) breaking near-ties toward shorter lag (higher
      F0, D3/D5) → voicing gate (NCC ≥ 0.30). NaN-denominator → NCC = 0.0. Make 1.1 pass. No
      `unwrap`/`expect`.
- [x] 1.3 Sample-rate handling: `detect_f0` SHALL derive the downsample factor and lag range from
      the `sample_rate` argument dynamically (`DOWNSAMPLE = sample_rate / 4000`), not hardcode
      16 kHz → 4 kHz. Write a failing test: 240 Hz sine at 48 kHz → ~240 Hz (the Tauri pipeline
      records at 48 kHz; `sherpa_adapter` resamples internally to 16 kHz before calling
      `detect_f0`, but the function's contract must not assume a single rate — a panic-`assert`
      on 48 kHz would crash the diarization pipeline if a future refactor passes the wrong rate).
      Do NOT use `assert!(sample_rate == 16000)`; dynamic derivation is mandated.

## 2. F0 corrective layer (pure computation — deterministic, no I/O; adversarial test-first)

- [x] 2.1 Write a failing test for `correct_labels_by_f0(labels, samples, sample_rate, chunks,
      centroids, params) -> Vec<u32>` — the **clean-meeting no-op** (D6): synthetic
      labels where every chunk's F0 agrees with its cluster's median → returned labels are
      identical to input. Assert zero reassignments.
- [x] 2.2 Write a failing test — **absorption recovery on synthetic data**: 4 chunks in a
      120 Hz-register cluster whose F0 is ~240 Hz (the absorbed speaker's voice), plus a 240 Hz
      cluster that exists in the same run. After correction, those 4 chunks are relabelled to
      the 240 Hz cluster. Assert the reassignment count and target labels.
- [x] 2.3 Write a failing test — **all-same-register is a no-op**: clusters with medians 140 Hz
      and 150 Hz (both below `F0_REGISTER_BOUNDARY`) → zero reassignments (the "opposite sides
      of the boundary" conjunct fails).
- [x] 2.4 Write a failing test — **unvoiced chunks are skipped**: chunks with NCC < 0.30
      (silence/noise) are never reassigned, regardless of register mismatch.
- [x] 2.5 Write a failing test — **boundary-register no-op (makes-worse guard)**: two clusters
      with medians 175 Hz and 185 Hz (straddling the 180 Hz boundary), chunks with F0 jitter
      ±15 Hz around their own cluster's median → zero reassignments. Proves the tolerance
      constants (30 Hz match, 50 Hz disagreement) are tight enough that near-boundary registers
      don't trigger false reassignments. If someone later loosens `F0_MATCH_TOLERANCE` to 50 Hz,
      this test fails.
- [x] 2.6 Write a failing test — **cluster count never increases**: after correction, the set of
      distinct labels in the output is a subset of the input's distinct labels. Also test
      `labels = []` and `labels = [0, 0, 0]` (single-cluster / empty input → no-op, no panic).
- [x] 2.7 Write a failing test — **determinism + tie-break label**: call `correct_labels_by_f0`
      twice on identical input → byte-identical output. Include a crafted input with two clusters
      equidistant from a chunk's F0 (near-tie under float rounding) → both runs pick the same
      winner, AND that winner is the SMALLEST-label cluster (not just agreement across runs — a
      largest-label winner would pass byte-identical but violate D4). Assert the specific label
      value. The implementation MUST sort cluster keys before scanning (D4 tie-break).
- [x] 2.8 Write a failing test — **creaky-voice chunk does not cause spurious reassignment**:
      synthetic waveform with alternating period lengths (jitter >20%, simulating vocal fry) →
      either NCC < 0.30 (skipped) or F0 within the chunk's own register. Assert no mid-register
      F0 output that would trigger a false reassignment.
- [x] 2.9 Implement `correct_labels_by_f0` per design D2/D4: build per-cluster F0 profiles
      (median of voiced members, ≥3 required), sort cluster keys and scan in label order, apply
      the three-conjunct reassignment criteria (no octave guard — D4/D5), deterministic
      tie-break (smallest label). Centroids are read-only (D2 — labels only). Make 2.1–2.8 pass.
- [x] 2.10 Write a failing test — **≥3 voiced-member exemption (both directions)**: cluster A
      (2 voiced members at 240 Hz, below threshold) + cluster B (≥3 voiced members at 120 Hz).
      (a) Chunks in A whose F0 is ~120 Hz are NOT reassigned to B — A is exempt as giver (its
      unreliable profile means the disagreement conjunct cannot be safely evaluated). (b) Chunks
      in B whose F0 is ~240 Hz are NOT reassigned to A — A is exempt as receiver (no reliable
      target profile to match against). Assert zero reassignments in both directions.
- [x] 2.11 Write a failing test — **centroids are unchanged**: after `correct_labels_by_f0`
      reassigns N>0 chunks, the `centroids` HashMap is byte-identical to the input (deep-equal
      all key→vector pairs). The `&` borrow enforces read-only at the type level; this assertion
      is defense-in-depth against a future signature change to `&mut`.

## 3. Wire into the pipeline

- [x] 3.1 Write a failing test in `sherpa_adapter.rs`: given synthetic audio with two
      pitch-distinct speakers where one is absorbed by embedding contamination (construct a
      scenario where clustering assigns the quieter speaker's chunks to the louder one),
      `process()` returns segments where both speakers are present throughout — the quieter
      speaker's late-half duration is ≥ 50% of her early-half duration (a single 3s chunk would
      trivially pass "not absent"; this threshold catches a real recovery). (This is the
      integration-level absorption adversarial from the spec's "Out of scope" note, now
      unblocked.)
- [x] 3.2 Insert `correct_labels_by_f0` into `process()` immediately after
      `smooth_to_fixed_point` and before segment coalescence (D1). Pass `(labels, samples,
      sample_rate, chunks, centroids, params)` — `samples` and `sample_rate` are the
      decoded-audio buffer and `DIARIZATION_SAMPLE_RATE` already in scope inside `process()`.
      `timestamps` is NOT passed (D2 — F0 correction is per-chunk, no temporal info needed; it
      is not in scope at the insertion point anyway). Centroids are read-only (D2). Make 3.1 pass.
      **(2026-07-10: gated off behind `F0_CORRECTION_ENABLED = false` after §4 real-data failure;
      the call site and code are retained for reference but do not execute in production.)**
- [x] 3.3 Remove the `cluster_by_average_linkage` function and its test call (D12 cleanup —
      disproven exploration branch). Remove `detect_f0_autocorr` from `commands.rs` (exploration
      scaffolding — bare `step_by(4)` with no boxcar, dead code once §1.2 lands the production
      `detect_f0`). Remove the F0-diagnostic and average-linkage blocks from
      `test_cde5c264_stage_trace_diagnostic` (exploration scaffolding); keep the core stage-trace
      as a `#[ignore]` regression guard.

## 4. Real-data validation (#[ignore] tests)

- [~] 4.1 Extend `test_cde5c264_stage_trace_diagnostic` (or add a companion) to run the full
      pipeline WITH `correct_labels_by_f0` and assert ALL rails:
      **(2026-07-10: FAILED — FLOOR UserB late 30s < 600s; EARLY-HALF GUARD 1078→820s = 24% theft.
      See design.md "⚠ REAL-DATA VALIDATION FAILED". The test is retained as a regression guard
      documenting the failure; it will fail until a non-circular F0 approach exists.)**
      - **Floor:** absorbed speaker's late-half duration ≥ 600s (recovery from ~26s).
      - **Ceiling:** absorbed speaker's late-half duration ≤ 1800s (no overshoot — D13 measured
        1381s recoverable as an UPPER BOUND using the looser `f0 ≥ 190` criterion; production
        `|f0 − 241| < 30` may recover less. 1800s gives 26% headroom over the upper bound. If
        the fix balloons past this, it is stealing Speaker 2's chunks, not recovering UserB's).
      - **Absorber guard:** Speaker 2's late-half duration is not reduced by more than 15%
        (symmetric regression guard — confirms the fix didn't trade one absorption for another).
      - **Early-half guard:** absorbed speaker's early-half duration not reduced >10% (no
        regression on correctly-assigned early speech).
      - **Per-chunk F0-register assertion (critical):** for every chunk whose label changed FROM
        Speaker 2's cluster TO UserB's cluster, assert `detect_f0(chunk_audio)` is within 30 Hz
        of UserB's cluster median (i.e. the fix moved the RIGHT chunks, not just A duration
        target). A fix that moves Speaker 2's true chunks into UserB's cluster could pass the
        four duration rails while making diarization worse — this assertion prevents that.
      - **Contamination diagnostic:** log each cluster's voiced-member count, F0 median, and
        contamination ratio (fraction of voiced members whose F0 is on the opposite side of 180 Hz
        from the median). Assert the absorber cluster's contamination < 40% (if higher, the median
        has shifted and recovery may be blocked — see design D4 median-contamination risk).
      - **Re-validation note:** the 600s/1800s thresholds were calibrated against the D13
        diagnostic which used unfiltered decimation (no boxcar). The production `detect_f0` adds a
        4-tap boxcar that may shift F0 values and voiced/unvoiced classifications. If the rails
        fail after §1.2 is implemented, adjust the floor/ceiling based on the filtered recovery
        figures (do NOT loosen the per-chunk assertion or contamination guard to make them pass).
      Run on prod DB (read-only).
- [~] 4.2 Add a regression `#[ignore]` test on meeting `95db` (3-speaker, known-good): speaker
      count stays 3, no speaker collapses by >15% between halves (a 50% threshold lets a
      near-absorption slip through on a "near-no-op" meeting — 15% catches a real regression),
      total reassignment count is small (the embeddings were already right → near-no-op).
      **(2026-07-10: FAILED — F0 correction collapsed 95db from 3→2 speakers. Confirms the
      approach is harmful, not just ineffective, on real data.)**

## 5. Property test

- [x] 5.1 Add a `proptest` for `correct_labels_by_f0`: for any valid set of labels, samples,
      sample_rate, chunks, and centroids (within defined bounds), with samples generated as sine
      waves at frequencies aligned or misaligned with cluster medians (NOT white noise — random
      noise makes every chunk unvoiced and the proptest trivially passes without exercising the
      correction logic), ALL of the following hold:
      - **Deterministic:** same input → byte-identical output across runs.
      - **Cluster count never increases:** output distinct-label set ⊆ input's.
      - **Idempotent (for clusters with ≥10 voiced members):**
        `correct_labels_by_f0(correct_labels_by_f0(x), x) == correct_labels_by_f0(x)` — running
        it twice moves the same chunks as running it once. Restricted to ≥10-member clusters
        because small clusters (3–4 members) can have their median shifted by a single
        reassignment, crossing the strict `< 30` boundary for a borderline chunk (see design D4
        idempotency note). For small clusters, idempotency is approximate.
      - **Reassignment validity:** every reassigned chunk's F0 matches the target cluster's
        median within `F0_MATCH_TOLERANCE` (30 Hz). This replaces the earlier "variance
        reduction" property, which was too strong — multi-source reassignment (a cluster gaining
        chunks from two different sources at slightly different F0s) can legitimately widen
        within-cluster variance without indicating a bug.

## 6. Gate + archive

- [ ] 6.1 Run `cargo test -- --include-ignored` (unit + #[ignore] real-data tests on cde5c264 +
      95db). **BLOCKED: §4.1/§4.2 fail by design (the approach is disproven). Unit + §5.1 proptest
      pass. Do not run §4 as a gate — they document the failure.**
- [ ] 6.2 Run the full §7 merge gate in parallel: `cargo test`, `pytest backend/`, `pnpm test`,
      `pnpm lint`. **(Partial: pytest 6/6 ✓, pnpm lint ✓, pnpm test 243/244 [1 pre-existing flaky
      ESLint timeout], cargo sherpa_adapter 76/76 ✓. Full cargo test --lib not yet run.)**
- [ ] 6.3 Assess smoke-test need: this is a backend diarization change with no user-visible UI
      behavior change — confirm no smoke spec is required, or add one if the reviewer disagrees.
      **(No smoke needed — Rust backend change, no UI.)**
- [ ] 6.4 Re-read `specs/speaker-diarization/spec.md` and `design.md`; amend the temporal-
      coherence smoothing requirement's "Out of scope" note to reference the F0 corrective layer
      as the established fix; then `/opsx:archive`. **BLOCKED: F0 correction is NOT the established
      fix — it failed. Do NOT archive. The "Out of scope" note in the canonical spec correctly
      remains: sustained absorption is unfixable from mixed-mono audio; the preventive fix is
      channel separation (a separate change).**
