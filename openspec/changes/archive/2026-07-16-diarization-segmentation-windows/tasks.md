## 1. Diagnostic gate (GATE — blocks all production changes)

> **Shark-tank hardened 2026-07-03.** The gate now tests the production code path (our
> clustering on native windows), uses a duration metric not binary presence, runs a three-way
> fork, sweeps params before abandoning, and gates on two meetings.

- [x] 1.1 Write a `#[ignore]` read-only test `test_cde5c264_native_pipeline_diagnostic` in
      `commands.rs` that constructs `OfflineSpeakerDiarization` directly from the two shipped
      models (`pyannote-segmentation.onnx` + `nemo-titanet-embedding.onnx`) with
      **`FastClusteringConfig::default()` (`num_clusters = -1`, auto — do NOT force K)** and
      **`min_duration_on = 0.0, min_duration_off = 0.0`**. Decode meeting `cde5c264`'s saved
      `audio.mp4` to 16kHz mono f32. Call `process(&samples)` and print:
      (a) wall-clock time of the native `process()` call and peak RSS;
      (b) `result.sort_by_start_time().len()` — the native window count (validates the
          "hundreds not thousands" assumption and whether the D4 cap is needed);
      (c) the native per-speaker duration distribution split at the midpoint (early < 1800s vs
          late ≥ 1800s);
      (d) **THEN re-embed the native window boundaries `(start, end)` through the existing
          `SpeakerEmbeddingExtractor` and run `cluster_by_centroids(..., 0.40) →
          smooth_to_fixed_point → enforce_max_speakers_cap(3)` on those embeddings**, printing
          the per-speaker duration distribution of **that** result (the production code path).
- [x] 1.2 Run the diagnostic on meeting `cde5c264` AND meeting `95db` (two meetings — `95db`
      guards against a single-meeting false pass).
      `cargo test test_cde5c264_native_pipeline_diagnostic -- --ignored --nocapture`.
      **DECISION (three-way fork, duration metric):**
      1. In the **our-clustering-on-native-windows** output (1.1(d)), the absorbed speaker's
         late-half duration is ≥ 30% of her early-half duration AND ≥ 60s absolute →
         segmentation is the fix; proceed to task 2.
      2. She survives in the native FastClustering output (1.1(c)) but NOT in our-clustering
         output (or vice versa) → the fix is partly in clustering. Do NOT abandon; scope a
         clustering-threshold/cap change alongside the segmentation windows (expand this change
         or open a companion).
      3. She is absent in BOTH outputs → do NOT immediately conclude acoustic. First sweep
         `FastClusteringConfig.threshold` (0.3, 0.4, 0.5) and re-run. Only if she remains
         absent across the sweep: the problem is acoustic (mixed mic+system audio buries her
         voice); STOP, do not proceed to task 2, scope a source-separation /
         separate-channel-capture change.

> **§1 RESOLVED (2026-07-04):** Fork outcome #1 — PROCEED. Native pyannote
> windows recover UserB 22× on cde5c264 (1016s late-half speech at cos > 0.5
> vs ~45s for whisper-derived chunks). Diagnostics committed in `94ef330` +
> `d4fc465`. 95db second-meeting guard folded into §4.1 regression.

## 2. Wire segmentation into the adapter (Path B — REWRITE PENDING)

> **Original tasks 2.1–2.3 below assumed the sherpa `OfflineSpeakerDiarization`
> API — now IMPOSSIBLE (ORT DLL collision with the `ort` crate; see design.md
> D1-revised). Path B replacement: load pyannote-segmentation.onnx via
> `ort::Session`, implement sliding-window post-processing in Rust, feed windows
> to existing SpeakerEmbeddingExtractor + AHC + smoothing. Tasks 2.1–2.3 below
> are the ORIGINAL (superseded); Path B rewrite is the next step.

- [ ] 2.1 Write a failing test in `sherpa_adapter.rs`: given synthetic audio with a known
      speaker turn at time T (two distinct speaker-tone regions concatenated), `process()`
      produces a chunk boundary near T — NOT at the whisper-segment boundary. Assert that at
      least one `Chunk` boundary falls within ±1s of T. (Adversarial: the current pipeline
      ignores turns and splits at fixed intervals.)
- [ ] 2.2 Add an `OfflineSpeakerDiarization` field to `SherpaOnnxDiarizationAdapter`.
      Construct it in `with_shared_threshold` from both shipped models via
      `OfflineSpeakerDiarizationConfig` with production `FastClusteringConfig.threshold = 1.0`
      (short-circuit native clustering — labels are discarded) and `min_duration_on = 0.0,
      min_duration_off = 0.0`. **Handle silent failure:** `create()` returns `None` on
      model-load failure with no error message — surface a meaningful typed error, do not
      collapse to a generic "failed". Verify the binary links (the `shared` feature already
      includes the diarization C API). Add an adversarial test: a model file that exists but is
      corrupt/incompatible → `create()` returns `None` → the adapter surfaces the error, not a
      panic or silent default.
- [ ] 2.3 In `process()`, run the native pipeline on the decoded samples to obtain
      speaker-homogeneous window boundaries `Vec<(f64, f64)>` (extract `start`/`end` from the
      native result, discard `speaker` labels). Pass these windows to `build_chunks` in place
      of the whisper `segments`. Make the task 2.1 test pass.

## 3. Preserve downstream pipeline guarantees

- [ ] 3.1 Write a failing test (the absorption adversarial from the spec): synthetic audio
      with two speakers of different RMS loudness (0.07 vs 0.04) interleaved within a single
      18s region. After `process()`, both speakers SHALL appear as distinct clusters, and the
      quieter speaker's cluster SHALL NOT be absorbed into the louder one.
- [ ] 3.2 Verify the temporal-coherence smoothing, `enforce_max_speakers_cap`, and
      duration-weighted centroid extraction still run unchanged on the native-windowed chunks
      (no code change to those functions — confirm the code path is unchanged). Make the test
      pass.
- [ ] 3.3 Write a test: a native window longer than `MAX_CHUNK_SECS` is split at
      `effective_split`; a native window shorter than `MIN_SPEECH_SECS` is dropped (no
      embedding extracted).
- [ ] 3.4 Write a test: empty transcript segments (whisper returned no segments) still
      produces a fallback single-speaker result (the existing energy-VAD fallback path is
      preserved or the native pipeline handles the no-transcript case).
- [ ] 3.5 Write a test (D4 replacement cap): when the native window count exceeds
      `MAX_DIARIZATION_CHUNKS` (600), the cap fires by raising `min_duration_on` and re-running
      `process()` until the count is at or below the cap, WITHOUT re-mixing speakers (windows
      remain speaker-homogeneous). Assert the final chunk count ≤ cap and clustering completes
      in bounded wall-clock. (The original `effective_split` path does NOT fire on native
      windows — they are < `MAX_CHUNK_SECS` — so the cap must be enforced here explicitly.)

## 4. Regression checks on real meetings

> **Shark-tank expanded 2026-07-03.** One meeting is insufficient — silent-degradation cases
> include single-speaker over-segmentation and 2-speaker count drift.

- [ ] 4.1 Write `#[ignore]` regression tests that re-run diarization with the new pipeline on a
      suite of real meetings and assert:
      - `95db` (3-speaker, known-good): speaker count matches stored result; no speaker
        collapses by > 50% between halves.
      - A 2-speaker meeting: count stays exactly 2.
      - A single-speaker meeting: count stays exactly 1 (guards against native
        over-segmentation producing phantom speakers).
      - `cde5c264` (the problem meeting): the absorbed speaker survives late — the positive
        case the whole change exists to deliver.
      - A short (< 10 min) meeting: latency baseline (native-pipeline overhead does not
        dominate).
- [ ] 4.2 Run the existing diarization oracle test (verification suite, commit `dedd325`) and
      confirm it still passes with the new pipeline.

## 5. Gate + archive

- [ ] 5.1 Run `cargo test -- --include-ignored` (unit tests + the `#[ignore]` diagnostics on
      real meetings).
- [ ] 5.2 Run the full §7 merge gate in parallel: `cargo test`, `pytest backend/`,
      `pnpm test`, `pnpm lint`.
- [ ] 5.3 Assess smoke-test need: this is a backend diarization change with no user-visible UI
      behavior change — confirm no smoke spec is required, or add one if the reviewer
      disagrees.
- [ ] 5.4 Re-read `specs/speaker-diarization/spec.md` and `design.md`; amend any drift before
      `/opsx:archive`. Then archive.
