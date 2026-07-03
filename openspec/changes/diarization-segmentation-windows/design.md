## Context

Meetily-flash's diarization has two known failure classes on multi-speaker meetings:

1. **Flicker** (FIXED, on `main` via `cdbaf54`) — per-chunk independent clustering caused rapid
   speaker-label switching. Fixed by the temporal-coherence smoothing pass.
2. **Absorption** (THIS CHANGE) — a quieter speaker vanishes mid-meeting. On meeting
   `cde5c264` (3 speakers, 70 min, 2026-06-22), the quieter third speaker disappears after
   ~min 30; her words remain in the transcript (whisper transcribed 2937s/3000s of the late
   portion) but are attributed to the louder "Speaker 2".

**Root cause (code-confirmed across 3 read-only diagnostic tracts on prod data):**

- `build_chunks` (`sherpa_adapter.rs:309`) carves diarization chunks **only** from whisper
  transcript segment boundaries.
- Long whisper segments (~18s avg in the problem meeting) are split at fixed `effective_split`
  intervals (6.0s) with **zero speaker-turn awareness** — literally `pos += chunk_samples`
  (line 364).
- These windows span multiple speakers; the louder voice dominates each chunk's embedding
  (third speaker RMS 0.071 vs absorbed speaker 0.036).
- The `pyannote-segmentation.onnx` model — designed to detect speaker turns and produce
  speaker-homogeneous windows — is shipped + file-existence-validated (`commands.rs:336`) but
  **never wired into the pipeline**. The adapter struct (`sherpa_adapter.rs:83`) has no field
  for it; the path is validated and discarded.

Tract 3 proved this is **not** embedding degradation: all 3 speakers' embeddings are stable cos
0.80–0.87 early→late. The embeddings are correct; the **windows** are wrong.

**The fix:** use the segmentation model to produce speaker-homogeneous windows. sherpa-onnx
1.13.3 (already pinned, `Cargo.toml:141`) exposes `OfflineSpeakerDiarization`
(`offline_speaker_diarization.rs`) — a native pipeline wrapping the segmentation model +
embedding model + fast clustering. The `pyannote-segmentation-3.0.onnx` we already ship is
exactly what it expects. One call: `process(samples: &[f32])` → speaker-homogeneous
`{start, end, speaker}` segments.

## Goals / Non-Goals

**Goals:**
- Fix absorption: a quieter speaker who shares whisper transcript segments with a louder
  speaker SHALL remain a distinct speaker throughout the meeting.
- Wire the already-shipped segmentation model into the pipeline (currently dead weight).
- Preserve every accumulated diarization fix: temporal-coherence smoothing (flicker),
  max_speakers cap, duration-weighted centroid storage, cross-meeting matching.

**Non-Goals:**
- Do NOT replace the clustering pipeline (`cluster_by_centroids → smooth → cap`). Our
  clustering stays; only the chunking **input** changes.
- Do NOT change token-level text alignment (the "Token-level timestamps" requirement is
  untouched).
- Do NOT add source separation or separate-channel mic/system capture. That is only relevant
  if the diagnostic (task 1) proves the mixed audio itself is acoustically unrecoverable.
- Do NOT change the `DiarizationPort` trait signature or the caller in `commands.rs`.

## Decisions

> **Shark-tank validated 2026-07-03.** Four independent reviewers (general-purpose agents,
> read-only, cold-started against the code) converged on PROCEED-WITH-AMENDMENTS. Two
> amendments rose to "critical" and are folded into D3 (clustering-mismatch) and D4
> (cap-mechanism) below. The standalone-segmentation claim in D1 was independently verified by
> two sharks.

### D1: Run native `OfflineSpeakerDiarization`, consume only its window boundaries

The segmentation model is exposed in the Rust binding **only** bundled inside
`OfflineSpeakerDiarization` — there is no standalone segmentation-model type. **Verified
2026-07-03 by two independent sharks:** the `sherpa-onnx-sys` C FFI exposes exactly 8
diarization functions, all namespaced `SherpaOnnxOfflineSpeakerDiarization*`; grep for
"Segmentation" across the sys crate returns only config structs, no runtime type. The hybrid
design is therefore forced, not a shortcut.

Running the full native pipeline means its internal `FastClustering` does work we immediately
discard. **Production sets `FastClusteringConfig.threshold = 1.0`** (or `num_clusters: 1`) so
native clustering short-circuits to a single cluster — the labels are thrown away regardless,
so this minimises wasted compute. (The diagnostic in task 1 runs native at defaults to compare
native-clustering output vs our-clustering output — see D3.)

The nemo_titanet embedding model is loaded **twice in RAM**: once inside the native pipeline
(internal, unavoidable without standalone segmentation FFI), once as our standalone
`SpeakerEmbeddingExtractor`. Task 1 measures peak RSS, not just wall-clock.

**Alternatives considered:**
- *Standalone C segmentation FFI* — rejected: no standalone type exists (verified), and writing
  custom FFI to reach the internal segmentation is over-complex.
- *Embedding-delta turn detection* (sliding 1–2s sub-windows inside each Whisper segment, split
  on cosine-similarity drop) — rejected: it reuses models we already load (no native pipeline,
  no double-embedding), but it is a window-level heuristic vulnerable to sub-window
  contamination and threshold tuning, versus pyannote's frame-level neural turn detector trained
  with temporal context. For the specific failure case (absorbed speaker RMS 0.036 buried in
  0.071 segments), the trained model is more reliable. This is the cheapest credible
  alternative and is held in reserve if the native-pipeline latency proves unacceptable (Risks).
- *Whisper word-level timestamps as turn boundaries* — rejected: `whisper_engine.rs:553` sets
  `set_no_timestamps(true)`, and word timestamps measure transcription timing, not speaker
  identity.

### D2: Discard native labels; keep our own clustering

We discard the native pipeline's speaker labels and run our own
`cluster_by_centroids → smooth_to_fixed_point → enforce_max_speakers_cap` on re-embedded
windows.

**Rationale (corrected 2026-07-03):** the native pipeline does not expose per-window embeddings,
so we must re-embed the windows regardless — to store duration-weighted centroids for
cross-meeting speaker matching. Once we are re-embedding, running our own clustering is
essentially free and preserves the accumulated fixes (temporal-coherence smoothing, max_speakers
cap, centroid extraction). The prior framing ("loses smoothing/cap/centroids") was imprecise —
those passes *could* run on native labels; the binding constraint is the missing per-window
embeddings.

**Alternative considered (full native pipeline):** trust the native labels entirely, skip our
clustering. Rejected — higher regression risk, loses centroid storage. Revisit only if the
double-embedding cost proves significant (task 1 measures).

### D3: Diagnostic gates all production code changes (task 1)

Before wiring anything into the adapter, task 1 runs the native pipeline on the problem
meeting's saved `audio.mp4` in a `#[ignore]` test. **The gate tests the production code path,
not the native labels** (shark-tank critical fix): the diagnostic (a) prints the native
per-speaker distribution, then (b) **re-embeds the native window boundaries through our
`SpeakerEmbeddingExtractor` and runs `cluster_by_centroids(0.40) → smooth →
enforce_max_speakers_cap(3)`**, printing *that* distribution. The proceed/abandon decision
rests on the our-clustering-on-native-windows output — because production discards native
labels and uses our clustering, a gate that does not test our clustering can green-light a fix
that production re-absorbs.

The decision is a **duration ratio, not binary presence** (shark-tank fix): the absorbed
speaker's late-half speech duration must be ≥ 30% of her early-half duration AND ≥ 60s
absolute. Binary "appears in late" admits a false pass (e.g. 47 min early, 4 s late).

**Three-way fork (shark-tank fix):**
1. Absorbed speaker survives in **our-clustering-on-native-windows** → segmentation is the fix;
   proceed to task 2.
2. She survives in native FastClustering output but NOT in our-clustering output (or vice
   versa) → the fix is partly in clustering. Do NOT abandon; scope a clustering-threshold / cap
   change alongside the segmentation windows.
3. She is absent in BOTH → do not immediately conclude "acoustic, abandon." First sweep
   `min_duration_on`/`min_duration_off` to 0.0 and the FastClustering `threshold`; only if she
   remains absent across the sweep is the problem deemed acoustic (the mixed mic+system audio
   buries her voice), and the change is abandoned in favour of source separation or
   separate-channel capture.

`FastClusteringConfig.num_clusters` must be left at default `-1` (auto); a positive value
forces K and rigs the test. The gate runs on **two meetings** (`cde5c264` + `95db`); a single
meeting cannot distinguish "segmentation fixes absorption" from "fixes absorption on this
audio."

### D4: Chunk-count cap for native windows (replaces the `effective_split` safety valve)

**The original D4 design was broken per the code** (shark-tank critical fix):
`build_chunks` (`sherpa_adapter.rs:332-347`) applies `effective_split` coarsening **only** in
the `dur > MAX_CHUNK_SECS` (10s) branch. Native pyannote windows are speaker-homogeneous turns
of typically 1–8s, so they take the one-chunk path and the cap never fires. A turn-heavy 70-min
meeting producing 800+ windows would yield 800+ chunks, blowing the O(n²) clustering budget
that `MAX_DIARIZATION_CHUNKS=600` was meant to protect.

**Replacement mechanism:** enforce the cap **explicitly after native windowing**, independent
of `build_chunks`' split logic. If the native window count exceeds `MAX_DIARIZATION_CHUNKS`,
coarsen by raising the native `min_duration_on` and re-running `process()` (pyannote
re-segments into fewer, longer, still-speaker-homogeneous windows) — this preserves window
homogeneity, unlike blind adjacent-window merging which would re-mix speakers. As a
last-resort fallback, temporally-adjacent windows may be merged. The `effective_split` path
remains for any window that genuinely exceeds `MAX_CHUNK_SECS`, but is no longer relied upon
as the cap.

## Risks / Trade-offs

- **[Native pipeline adds wall-clock latency — highest user-visible risk]** → Estimated
  0.05–0.2× realtime for pyannote segmentation = ~3.5–14 min wall-clock on a 70-min meeting
  (shark-tank estimate; task 1 measures the real figure). The pipeline runs on a blocking
  thread (`spawn_blocking`) so it never freezes the UI, but `rediarize_meeting` and
  post-recording diarization block on it. **Fallback gate (shark-tank fix):** if task 1
  measures > 60s wall-clock for a 70-min meeting, scope a downsampled-path option (run the
  native pipeline on 8kHz-decimated audio for segmentation boundaries only, then re-embed
  windows at 16kHz) before proceeding to task 2.
- **[Double-embedding cost]** → The dominant cost is the native pipeline's INTERNAL work
  (segmentation + ~4200–8400 internal embedding passes on a 70-min meeting, all discarded),
  not our re-embed step (which runs on the ~hundreds of post-clustering windows). Setting
  production `threshold=1.0` (D1) avoids wasted native clustering. Task 1 measures peak RSS
  and wall-clock.
- **[Diagnostic shows native pipeline also absorbs the speaker]** → D3's three-way fork
  handles this: sweep params first; only abandon if she remains absent across the sweep.
- **[Native window count exceeds perf cap on a turn-heavy meeting]** → D4's replacement cap
  (raise `min_duration_on` + re-run) handles this; the original `effective_split` path did not
  fire on native windows.
- **[Regression on meetings where the current pipeline works]** → Task 4 re-runs diarization
  on a regression suite (shark-tank expansion): a known-good 3-speaker meeting (`95db`), a
  2-speaker meeting (count stays 2), a single-speaker meeting (native over-segmentation risk —
  count stays 1), the problem meeting `cde5c264` (absorbed speaker survives late), and a short
  (<10 min) meeting (latency baseline).

## Migration Plan

- No data migration. Existing meetings' stored diarization is unchanged; only future
  diarization runs (and re-diarization of existing meetings) use the new pipeline.
- The `DiarizationPort::process` signature is unchanged, so the caller in `commands.rs` is
  unaffected.

## Open Questions

- What is the wall-clock cost and peak RSS of the native pipeline on a 70-min meeting? (Task 1
  measures.)
- The `min_duration_on` (0.3s) / `min_duration_off` (0.5s) defaults were previously open.
  **Resolved 2026-07-03 (shark-tank):** set both to `0.0` — our downstream `MIN_SPEECH_SECS =
  1.5` filter in `build_chunks` does all duration filtering, so the native defaults only
  silently drop 0.3–1.5s turns (precisely the short speaker turns we may need to recover). Zero
  cost, removes a silent data-loss path.
