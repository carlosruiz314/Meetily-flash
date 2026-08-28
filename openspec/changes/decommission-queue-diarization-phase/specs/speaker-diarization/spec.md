## MODIFIED Requirements

### Requirement: Transcript-timestamp-driven speaker diarization runs as a post-processing queue phase

After transcription completes — and after summarisation when an LLM provider is configured — the system SHALL run offline speaker diarization on the meeting's `audio.mp4` by invoking `run_diarization_for_meeting`. Diarization is not a transcription-queue phase: the queue's `JobPhase` ends at `Summarising`, and diarization is invoked automatically at the end of the import and Enhance/retranscription pipelines and on demand via the Speakers/reset commands. The diarization run SHALL:

1. Decode the audio to 16kHz mono f32 samples via `DecodedAudio::to_whisper_format()`
2. Read transcript timestamps from the `transcripts` table to define speech segments
3. Chunk each segment into pieces: on the success path the pieces are laid out by the pyannote change-points intersected into each Whisper speech region; `effective_split` applies only as the size guard for surviving segments longer than `MAX_CHUNK_SECS`, or as the full grid on the pyannote-model-missing/corrupt fallback (see the amendment below)
4. Extract a speaker embedding for each chunk via `SpeakerEmbeddingExtractor` (nemo_titanet; see model-selection requirement)
5. Cluster chunks using centroid-based agglomerative clustering with duration-weighted averaging. The clustering SHALL use a **cached** pairwise similarity scheme — the similarity between alive clusters is computed once and recomputed only for the newly-merged cluster on each merge, not via a full per-merge pairwise rescan — so that its total cost is bounded and it completes in bounded wall-clock time for any meeting length. The clustering SHALL run off the async executor (on a blocking thread) so it can never freeze the UI or block other work.
6. Merge short-duration speakers into their cosine-nearest larger cluster
7. Align transcript rows with diarization speaker segments

The clustering output (per-chunk labels and duration-weighted centroids) SHALL be identical regardless of the cached-similarity optimization internals — the optimization changes cost, not results. Diarization SHALL be skipped if no `audio.mp4` exists (e.g., `auto_save = false`). Diarization SHALL run on imported audio files via the same `run_diarization_for_meeting` path.

This requirement amends the canonical requirement of the same name. The canonical requirement's item 3 mandates the **effective-split chunk grid**: "Chunk each segment into pieces sized at the **effective split granularity** = `max(SPLIT_TARGET_SECS, speech_seconds / MAX_DIARIZATION_CHUNKS)` … Each piece remains within [`MIN_SPEECH_SECS`, `MAX_CHUNK_SECS`]." With this change the chunk grid is no longer the source of speaker-change-point boundaries on the success path; the in-process pyannote `ort::Session` (see the "Diarization segment granularity resolves speaker turns within Whisper segments" requirement below) supplies intra-region splits that `build_chunks` consumes INSTEAD of the effective-split grid. The canonical item 3's `effective_split` mandate is STRUCK as a boundary SOURCE on the success path; it SURVIVES in one narrow role: `build_chunks` still sub-divides any surviving segment LONGER than `MAX_CHUNK_SECS` (10s) at the effective-split granularity — which happens when a Whisper speech region has no interior pyannote change-points and therefore stays whole (e.g. a boundary-free monologue region). This is a size guard, not a competing boundary source. The only fallback remains pyannote-model-missing: when the segmentation model file is absent, `build_chunks` applies `effective_split` exactly as the canonical item 3 states, so the meeting still diarizes at coarse resolution. (There is no child-failure fallback — pyannote runs in-process on the shared `ort` runtime, so there is no subprocess whose spawn/crash/timeout/schema-mismatch failure could fire.)

The canonical "Short meeting is unaffected by the chunk cap" scenario asserts `effective granularity equals SPLIT_TARGET_SECS (3.0 s) — unchanged from before this change`. That assertion is RE-POINTED to the in-process pyannote boundary source: on a short (~10 min) meeting the pyannote model is present (the cap is not hit), and the per-region granularity is set by the pyannote change-points inside each Whisper segment — NOT by a fixed `SPLIT_TARGET_SECS` grid. The "chunk count is identical to a fixed-3 s chunker" clause no longer holds on the success path; the chunk count on the success path equals the count of pyannote change-points (capped). On the pyannote-model-missing fallback path the canonical assertion holds unchanged.

The canonical "Long meeting does not stall in clustering" scenario asserts "the effective split granularity is coarsened so the chunk count is at or below `MAX_DIARIZATION_CHUNKS`." That cap-enforcement mechanism is RE-POINTED to pyannote-boundary shedding: on a long meeting the cap is enforced once at the pyannote-boundary layer (uniform shed every k-th candidate by position, then merge sub-`MIN_SPEECH_SECS` survivors within their Whisper region — see the "Uniform shed-to-cap" scenario below), NOT by coarsening `effective_split`. The chunk count is bounded primarily by shedding the pyannote candidate set; because `effective_split` survives as the size guard for surviving segments longer than `MAX_CHUNK_SECS`, the count reaching clustering can modestly exceed `MAX_DIARIZATION_CHUNKS` (bounded ≈2× in practice — only boundary-free >10s regions contribute extra pieces). The canonical scenario's "bounded wall-clock time" and "clustering produced N speakers from M chunks" assertions hold unchanged. On the pyannote-model-missing/corrupt fallback path the canonical `effective_split` coarsening holds unchanged.

`FINE_SPLIT_SECS` (canonical default 2.0s) is referenced by the canonical "Diarization segment granularity resolves speaker turns within Whisper segments" requirement as the turn-granularity source ("A turn of approximately 2 seconds (the fine-split granularity `FINE_SPLIT_SECS`)..."). That role is STRUCK on the success path: turn granularity is now set by the pyannote change-points, NOT by `FINE_SPLIT_SECS`. `FINE_SPLIT_SECS` SURVIVES as the `refine_pass2` re-embedding window (`build_fine_chunks` re-chunks the full recording at `FINE_SPLIT_SECS` to assign each fine chunk to its nearest Pass-1 centroid) — it is no longer the granularity-defining constant but remains the Pass-2 re-chunk cadence. (A delta that left `FINE_SPLIT_SECS` mandated as the turn-granularity source alongside a pyannote-boundary requirement would self-contradict; this note reconciles the canonical reference.)

(A delta that leaves the canonical item 3 `effective_split` mandate in place alongside a pyannote pre-splitter requirement would make the canonical spec self-contradict — both cannot be the chunk-layout source simultaneously. This amendment removes that contradiction.)

#### Scenario: Diarization runs after Enhance or import transcription

- **WHEN** an import or Enhance/retranscription run finishes transcribing the meeting
- **THEN** diarization begins on the meeting's `audio.mp4` via `run_diarization_for_meeting`
- **AND** stale auto speaker labels and embeddings for the meeting are cleared before new labels are written

#### Scenario: Diarization is skipped when no audio file exists

- **WHEN** diarization is invoked for a meeting that has no `audio.mp4` (e.g., `auto_save = false`)
- **THEN** the run is skipped without an error surfaced to the user
- **AND** existing transcript labels are left untouched

#### Scenario: Diarization runs on imported audio

- **WHEN** an audio file is imported as a new meeting AND the import triggers transcription
- **THEN** diarization produces speaker labels for the imported audio via `run_diarization_for_meeting`
