## Why

In multi-speaker meetings, a quieter speaker can vanish from diarization mid-meeting
("absorption"). On meeting `cde5c264` (3 speakers, 70 min, 2026-06-22), the quieter third
speaker disappears after ~min 30 — her words remain in the transcript (whisper transcribed
2937s/3000s of the late portion) but are attributed to the louder "Speaker 2".

Root cause (code-confirmed, not theorised): diarization chunks are carved from **whisper
transcript segment boundaries** (`sherpa_adapter.rs::build_chunks`, line 309). Long segments
(~18s avg in this meeting) are split at fixed `effective_split` intervals (6.0s) with **zero
speaker-turn awareness** — `pos += chunk_samples`. These windows span 2+ speakers; the louder
voice dominates each chunk's embedding (third speaker RMS 0.071 vs the absorbed speaker 0.036),
burying the quieter one. The embeddings themselves are correct (all speakers stable cos
0.80–0.87 early→late); the **windows** are wrong.

The `pyannote-segmentation.onnx` model — designed to detect speaker turns and produce
speaker-homogeneous windows — is already downloaded and file-existence-validated
(`commands.rs:336`) but **never wired into the pipeline**. The adapter struct
(`sherpa_adapter.rs:83`) has no field for it; the path is validated and discarded. This is
project-blocking: diarization is unreliable for any meeting where speakers share a transcript
segment, which is most multi-speaker meetings.

## What Changes

- Wire the already-shipped `pyannote-segmentation.onnx` into diarization via sherpa-onnx's
  native `OfflineSpeakerDiarization` pipeline (available in the pinned dep, `sherpa-onnx`
  1.13.3). This pipeline runs the segmentation model over the audio and returns
  speaker-homogeneous window boundaries.
- These native window boundaries **replace** whisper transcript segments as the chunking input
  to `build_chunks`. Windows are now speaker-homogeneous, so each embedding represents a single
  voice instead of a mix.
- The existing embed → cluster → temporal-coherence-smooth → max_speakers-cap → store pipeline
  runs **unchanged** on these windows. All accumulated fixes (flicker smoothing, cap
  enforcement, centroid storage, cross-meeting matching) are preserved.
- Whisper transcript segments remain the source for **token-level text alignment** (unchanged —
  the "Token-level timestamps" requirement is not affected).
- The `MAX_DIARIZATION_CHUNKS` perf cap is **re-mechanised** (shark-tank critical fix): the
  original `effective_split` coarsening only fired for windows > `MAX_CHUNK_SECS` (10s), but
  native windows are 1–8s speaker-homogeneous turns, so the cap never triggered on them. The cap
  is now enforced **post-windowing** by raising the native `min_duration_on` and re-running
  `process()` until the window count is at or below the limit (preserving speaker homogeneity).
- The native pipeline's internal embedding/clustering work is **discarded** — only its
  segmentation window boundaries are consumed. This is deliberate: it lets us keep our own
  clustering (with temporal smoothing + cap) while gaining the segmentation model's
  speaker-turn awareness. The cost is a one-time native inference pass (measured in the
  diagnostic task).

## Capabilities

### Modified Capabilities

- `speaker-diarization`: The chunking step changes from whisper-transcript-segment-bound fixed
  splits to segmentation-model-driven speaker-turn-aware windows. The requirement currently
  titled "Transcript-timestamp-driven speaker diarization" is retitled to reflect that
  transcript timestamps drive **alignment**, while the segmentation model drives **chunking**.

## Impact

- **`sherpa_adapter.rs`** — adapter gains an `OfflineSpeakerDiarization` field (constructed from
  both already-shipped models); `process()` runs the native pipeline to obtain
  speaker-homogeneous window boundaries, then passes them to `build_chunks` in place of the
  whisper segments.
- **`commands.rs`** — adapter construction (line 369) passes both model paths to the native
  pipeline config in addition to the existing embedding extractor.
- **Performance** — adds a native inference pass (segmentation + ~4200–8400 internal embedding
  passes on a 70-min meeting, discarded) before our re-embed step. Estimated 0.05–0.2× realtime
  = ~3.5–14 min wall-clock on a 70-min meeting (shark-tank estimate; task 1 measures the real
  figure). Runs on a blocking thread so it never freezes the UI, but `rediarize_meeting` and
  post-recording diarization block on it. **If task 1 measures > 60s for a 70-min meeting, an
  8kHz-downsampled segmentation path is scoped before proceeding** (design.md Risks). Our
  re-embed operates on the ~hundreds of post-clustering windows, so our clustering cost is
  unchanged; production sets native `FastClusteringConfig.threshold = 1.0` to short-circuit the
  discarded native clustering. The nemo_titanet model loads twice in RAM (native-internal + our
  extractor); task 1 measures peak RSS.
- **Models** — no new downloads. Uses the already-shipped `pyannote-segmentation.onnx` +
  `nemo-titanet-embedding.onnx`.
- **No breaking API changes** — the `DiarizationPort::process` signature is unchanged; the
  caller in `commands.rs` is unaffected.
