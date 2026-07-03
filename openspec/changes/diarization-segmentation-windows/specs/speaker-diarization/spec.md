## MODIFIED Requirements

### Requirement: Transcript-timestamp-driven speaker diarization runs as a post-processing queue phase

After the transcription and summarisation phases complete, the system SHALL run offline
speaker diarization on the meeting's `audio.mp4` as a `Diarizing` phase in the transcription
queue. The diarization phase SHALL:

1. Decode the audio to 16kHz mono f32 samples via `DecodedAudio::to_whisper_format()`.
2. Run the native `OfflineSpeakerDiarization` pipeline (pyannote speaker-segmentation model +
   nemo_titanet embedding model, both already shipped) on the decoded audio to obtain
   **speaker-homogeneous window boundaries**. The segmentation model detects speaker turns;
   each returned window bounds a region dominated by a single speaker. The native pipeline's
   internal clustering labels are DISCARDED — only its window boundaries are consumed.
3. Chunk at the native window boundaries. A window longer than `MAX_CHUNK_SECS` SHALL be split
   at the **effective split granularity** = `max(SPLIT_TARGET_SECS, speech_seconds /
   MAX_DIARIZATION_CHUNKS)`; a window shorter than `MIN_SPEECH_SECS` SHALL be dropped. Because
   native windows are speaker-homogeneous turns (typically 1–8s, below `MAX_CHUNK_SECS`), the
   chunk count SHALL additionally be capped **post-windowing**: if the native window count
   exceeds `MAX_DIARIZATION_CHUNKS`, the native `min_duration_on` SHALL be raised and
   `process()` re-run until the count is at or below the cap (preserving speaker homogeneity),
   before chunking proceeds.
4. Extract a speaker embedding for each chunk via `SpeakerEmbeddingExtractor` (nemo_titanet;
   see model-selection requirement).
5. Cluster chunks using centroid-based agglomerative clustering with duration-weighted
   averaging. The clustering SHALL use a **cached** pairwise similarity scheme — the
   similarity between alive clusters is computed once and recomputed only for the newly-merged
   cluster on each merge, not via a full per-merge pairwise rescan — so that its total cost is
   bounded and it completes in bounded wall-clock time for any meeting length. The clustering
   SHALL run off the async executor (on a blocking thread) so it can never freeze the UI or
   block other queue work.
6. Merge short-duration speakers into their cosine-nearest larger cluster.
7. Align transcript rows with diarization speaker segments using token-level timestamps from
   the `transcripts` table (see the token-level-alignment requirement).

The clustering output (per-chunk labels and duration-weighted centroids) SHALL be identical
regardless of the cached-similarity optimization internals — the optimization changes cost, not
results. The diarization phase SHALL be skipped if no `audio.mp4` exists (e.g.,
`auto_save = false`). The diarization phase SHALL run on imported audio files using the same
queue path. Transcript timestamps drive **text alignment** (step 7); the segmentation model
drives **chunking** (step 2–3).

#### Scenario: Diarization runs after summarisation

- **WHEN** a queue job completes the `Summarising` phase successfully
- **THEN** the job transitions to `phase = "diarizing"` and diarization begins on the meeting's `audio.mp4`
- **AND** a `transcription-queue-changed` event is emitted with the updated phase

#### Scenario: Diarization runs directly after transcription when no summary provider

- **WHEN** a queue job completes the `Transcribing` phase AND no LLM provider is configured
- **THEN** the job transitions to `phase = "diarizing"` (skipping `Summarising`)
- **AND** diarization begins on the meeting's `audio.mp4`

#### Scenario: Diarization is skipped when no audio file exists

- **WHEN** a queue job reaches the `Diarizing` phase AND the meeting has no `audio.mp4` (e.g., `auto_save = false`)
- **THEN** the diarization phase is skipped
- **AND** the job transitions to `status = "done"`

#### Scenario: Diarization runs on imported audio

- **WHEN** an audio file is imported as a new meeting AND the import triggers transcription
- **THEN** the queue job includes the `Diarizing` phase after transcription/summarisation
- **AND** diarization produces speaker labels for the imported audio

#### Scenario: Quieter speaker sharing a transcript segment remains distinct

- **GIVEN** a meeting where two speakers share a whisper transcript segment (e.g., a louder
  remote speaker and a quieter local speaker in the same ~18s segment), and the quieter
  speaker is audible throughout the meeting
- **WHEN** diarization runs using segmentation-model-driven window boundaries
- **THEN** the quieter speaker SHALL appear as a distinct speaker in both the early and late
  portions of the meeting
- **AND** the quieter speaker's total attributed speech duration SHALL NOT collapse by more
  than 50% between the first half and second half of the meeting
- **AND** a `clustering produced N speakers` log line reflects that the quieter speaker was
  not absorbed into the louder speaker's cluster

#### Scenario: Segmentation model produces speaker-homogeneous chunk boundaries

- **GIVEN** a meeting audio where speaker turns occur at known timestamps
- **WHEN** the native `OfflineSpeakerDiarization` pipeline runs on the decoded audio
- **THEN** the returned window boundaries SHALL align with speaker-turn locations (not with
  whisper transcript segment boundaries)
- **AND** each window fed to the embedding step SHALL bound a region dominated by a single
  speaker

#### Scenario: Long meeting does not stall in clustering

- **GIVEN** a turn-heavy meeting whose native segmentation-model windows would exceed
  `MAX_DIARIZATION_CHUNKS`
- **WHEN** diarization runs the native windowing step
- **THEN** the native `min_duration_on` is raised and `process()` re-run so the window count is
  brought to or below `MAX_DIARIZATION_CHUNKS` BEFORE clustering (the `effective_split` path
  does not fire on native windows, which are typically 1–8s — below `MAX_CHUNK_SECS`)
- **AND** the clustering step completes in bounded wall-clock time (seconds, not minutes or
  hours)
- **AND** a `clustering produced N speakers from M chunks` log line is emitted with
  `M ≤ MAX_DIARIZATION_CHUNKS`

#### Scenario: Cached clustering is behaviour-identical to the naive rescan

- **GIVEN** the same set of chunk embeddings and the same merge threshold
- **WHEN** clustering runs via the cached-similarity implementation
- **THEN** the resulting per-chunk labels and duration-weighted centroids are identical to
  those produced by a full per-merge pairwise rescan (verified by a property test against a
  naive oracle kept under `#[cfg(test)]`)

#### Scenario: Clustering does not freeze the UI

- **GIVEN** a diarization run whose clustering step takes several seconds
- **WHEN** clustering executes
- **THEN** the async runtime and UI remain responsive because clustering runs on a blocking
  thread, not the executor

---
