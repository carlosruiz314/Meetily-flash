## Why

A quieter speaker vanishes mid-meeting ("sustained absorption") and the project is blocked on
it. On `meeting-cde5c264` (3 speakers, 83 min, 2026-06-22) the third speaker disappears after
~min 30 — her words stay in the transcript (Whisper transcribed them) but are attributed to the
louder "Speaker 2".

The root cause is now **established** (not theorised — code-confirmed across 6 diagnostic tracts
on prod data, `openspec/exploration/diarization-eend-poc-log.md` D8–D13):

1. **Mixed-audio chunks produce contaminated embeddings.** When two remote speakers are mixed on
   one system channel (pre-mixed by Zoom/Meet before Meetily captures anything), the louder voice
   dominates each chunk's TDNN embedding. The quieter speaker's voiceprint is buried.
2. **No clustering algorithm recovers this.** Stage-trace on the exact production path shows the
   quieter speaker is absorbed at the AHC step itself — 954s early / 48s late (ratio 0.05).
   Switching from centroid linkage to average linkage (the Python POC's algorithm) produces the
   **same** 48s late (D12). Smoothing and merge-short are innocent (marginal changes, D11).
3. **F0 (pitch) is the only surviving signal.** Pitch is a separate acoustic dimension from the
   embedding — vocal-fold vibration rate is preserved in the waveform even when the voiceprint is
   buried. D13 validated: of 423 late-half chunks the embeddings mis-assigned, **202 have F0 in
   the quieter speaker's register (≥190 Hz)** — **1381s (23 min) recoverable (upper bound)**.
   This figure uses the looser `f0 ≥ 190` diagnostic criterion; the production 3-conjunct
   criteria recover only F0 within 30 Hz of the cluster median, so actual recovery may be lower.
   The 600s spec floor provides margin.

The canonical spec already anticipated this: the temporal-coherence smoothing requirement
carries an explicit "Out of scope — sustained speaker absorption" note saying the root cause is
"not yet determined and is filed as a separate change; do NOT re-attempt a label-level fix for
sustained absorption without first establishing the cause." This change establishes the cause and
delivers the fix the spec deferred.

## What Changes

- Add an **F0 (pitch) corrective layer** to the diarization pipeline. After AHC clustering +
  temporal-coherence smoothing, compute per-chunk F0 via autocorrelation (downsampled 4× to
  4 kHz, NCC-normalised, voicing-gated). Build a per-cluster F0 profile (median of voiced
  members). Reassign any chunk whose F0 strongly matches a **different** cluster's register
  (three-conjunct criteria: F0 within 30 Hz of the target, >50 Hz from the current cluster, and
  the two registers on opposite sides of 180 Hz — no octave-error guard, which blocks recovery
  on the ~2:1 register ratio common for male/female voice pairs).
- **Labels only, not centroids.** F0-reassigned chunks get the target cluster's **label** but do
  **not** contribute to its stored centroid — their embeddings are contaminated by definition, so
  including them would corrupt the cross-meeting voice profile. The stored centroids remain the
  post-smoothing values (de-contaminated, as the existing spec already requires).
- The layer is a **pure computation** (deterministic, no I/O — `correct_labels_by_f0`) inserted
  after
  `smooth_to_fixed_point` and before segment coalescence in `sherpa_adapter.rs::process()`. No
  I/O, no new deps, no trait-signature change.
- **Clean-meeting no-op guarantee.** When every cluster's F0 profile is similar (all speakers
  share a pitch register) or every chunk already agrees with its cluster's profile, zero chunks
  are reassigned. A meeting where the embeddings already got everything right is unchanged.

## Capabilities

### Modified Capabilities

- `speaker-diarization`: Adds the F0 corrective-layer requirement; updates the temporal-coherence
  smoothing requirement's "Out of scope — sustained absorption" note to reference the now-
  established root cause and the F0 fix.

## Impact

- **`sherpa_adapter.rs`** — new `correct_labels_by_f0` function + `detect_f0` helper, called
  inside `process()` after smoothing. The existing `cluster_by_average_linkage` added during
  exploration (D12, disproven) is removed as cleanup.
- **`commands.rs`** — the stage-trace diagnostic test (`test_cde5c264_stage_trace_diagnostic`) is
  cleaned up: average-linkage comparison block and F0 diagnostic block are removed (they were
  exploration scaffolding); the core stage-trace remains as a `#[ignore]` regression guard.
- **No model changes** — F0 is computed from the raw waveform via autocorrelation. No new ONNX
  model, no new download.
- **No API changes** — `DiarizationPort::process` signature unchanged; caller in `commands.rs`
  unaffected.
- **Performance** — autocorrelation on downsampled audio adds <1s for an 83-min meeting (676
  chunks × ~8.3s each at 4 kHz, lag range 10–50). Runs on the existing blocking thread.
