# speaker-diarization Specification

## Purpose
TBD - created by archiving change speaker-diarization. Update Purpose after archive.
## Requirements
### Requirement: Transcript-timestamp-driven speaker diarization runs as a direct post-transcription step

**Amendment (sequenced after `decommission-queue-diarization-phase`):** diarization is not a transcription-queue phase; it runs as a direct post-transcription step invoked via `run_diarization_for_meeting` (import, re-transcription, and explicit Speakers re-run call sites). After the transcription (and, where configured, summarisation) steps complete for a meeting, the system SHALL run offline speaker diarization on the meeting's `audio.mp4`:

1. Decode the audio to 16kHz mono f32 samples via `DecodedAudio::to_whisper_format()` (decode SHALL run inside the same blocking closure as the rest of the pipeline)
2. Transcript rows are used for text alignment and textless-run detection ONLY — speech regions and turn boundaries derive from the pyannote per-frame activity per the run-assembly requirement
3. On the success path (pyannote segmentation model present), turn boundaries and labels derive from the run-assembly engine ("Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits") using a SINGLE pyannote inference pass; whisper-row/chunk-grid fragments are not the labeling unit and the effective-split grid is not a boundary source. `build_chunks`, the effective-split grid, temporal-coherence smoothing, and per-chunk clustering remain the pyannote-model-missing/corrupt fallback path, unchanged
4. Extract ONE speaker embedding per labeled piece via `SpeakerEmbeddingExtractor` (nemo_titanet; middle-12s slice for pieces >12s); pieces below the 1.5s floor are embedded only for attachment
5. Cluster the labeled pieces (threshold clustering at the configured merge threshold, most-isolated merge to the meeting's max_speakers cap, nearest-centroid refinement); clustering SHALL run off the async executor (on a blocking thread), SHALL be deterministic (ties by index/time order), and SHALL be bounded by the piece shed-to-cap so it completes in bounded wall-clock time for any meeting length
6. Attach unlabeled/ambiguous pieces per the run-assembly requirement
7. Align transcript rows with the derived turns (max-overlap assignment; content-preservation invariant) and persist per the existing `transcripts` schema plus the `continues_previous` fact

Diarization SHALL be skipped if no `audio.mp4` exists (e.g., `auto_save = false`). Imported audio uses the same pipeline. When the segmentation model file is absent, the run reports "speaker models not found" and skips (no labels are produced); when the model file is present but fails to load, the fallback chunk-grid path runs.

#### Scenario: Diarization runs after transcription

- **WHEN** the post-transcription pipeline reaches diarization (after summarisation where configured)
- **THEN** diarization begins on the meeting's `audio.mp4` via `run_diarization_for_meeting`
- **AND** progress is observable through the existing run/result reporting

#### Scenario: Diarization is skipped when no audio file exists

- **WHEN** the pipeline reaches diarization AND the meeting has no `audio.mp4` (e.g., `auto_save = false`)
- **THEN** the diarization step is skipped with no error surfaced

#### Scenario: Diarization runs on imported audio

- **WHEN** an audio file is imported as a new meeting AND the import triggers transcription
- **THEN** diarization produces speaker labels for the imported audio via the same pipeline

#### Scenario: Short meeting derives turns from pyannote speech runs

- **GIVEN** a meeting with ~10 minutes of speech AND the pyannote segmentation model is present on disk
- **WHEN** diarization runs
- **THEN** turn units are pause-delimited speech runs from the pyannote per-frame activity (NOT a fixed-grid chunking)
- **AND** the effective-split grid is not applied on this path

#### Scenario: Model absent skips; model corrupt falls back

- **GIVEN** the pyannote segmentation model file is absent from disk
- **WHEN** diarization runs
- **THEN** the run reports "speaker models not found" and produces no labels
- **GIVEN** instead that the model file is present but fails to load
- **WHEN** diarization runs
- **THEN** the fallback chunk-grid path (`build_chunks` effective-split grid, per-chunk embedding clustering, temporal-coherence smoothing) runs as before and produces labels
- **AND** no panic propagates to the user-facing flow

#### Scenario: Piece clustering completes in bounded time and never freezes the UI

- **GIVEN** an ~83-minute meeting bounded by the piece shed-to-cap (2000 embedded pieces)
- **WHEN** diarization runs
- **THEN** clustering completes in bounded wall-clock time (seconds, not minutes) under the stated bound
- **AND** the async runtime and UI remain responsive because decode/inference/clustering run on a blocking thread

#### Scenario: Clustering is deterministic

- **GIVEN** the same piece embeddings and the same thresholds
- **WHEN** clustering runs twice
- **THEN** labels and centroids are identical, with ties broken by index/time order

---

**Retirements introduced by this change (with the run-assembly engine on the success path; all retained on the fallback path):**

- Cached-similarity clustering internals and their behavior-identity scenario: retired with the per-chunk clustering on the success path. **Migration**: none — implementation detail of the fallback path, where the mandate and its property test remain in force.
- "Long meeting cap is enforced by pyannote-boundary shedding": superseded on the success path by the piece shed-to-cap (2000). **Migration**: `MAX_DIARIZATION_CHUNKS`/`shed_boundaries_to_cap` continue to govern the fallback path.
- "Merge short-duration speakers into their cosine-nearest larger cluster" (queue-phase item 6): superseded on the success path by the attachment rules. **Migration**: retained verbatim on the fallback path.
- `phase = "diarizing"` queue assertions: superseded by `decommission-queue-diarization-phase` transport. **Migration**: progress observable through `run_diarization_for_meeting` reporting. Two clauses of decommission's end state are carried forward unchanged by this delta's re-write and shall not be lost at archive: stale auto speaker labels/embeddings are cleared before new labels are written, and skipped runs leave existing labels untouched (except where the re-diarization requirement above explicitly changes manual-row handling on the explicit re-run path).
- **Amendments to untouched requirements (success-path scope)**: "Temporal-coherence smoothing prevents clustering contamination and per-chunk flicker" (canonical ~line 690), "Diarization segment granularity resolves speaker turns within Whisper segments" (~line 784), "Short chunks are not attributed to temporally-absent speakers" (~line 867), and "Short-duration noise speakers are merged into nearest cluster" (~line 93, whose `MIN_CLUSTER_FRAC` merge is superseded by the attachment rules) henceforth govern ONLY the fallback path; "Centroid embeddings are stored per speaker per meeting" (~line 250) is amended so that on the success path stored centroids derive from run/piece embeddings and contain no smoothing-refinement clause; "Token-level timestamps align transcript text with diarization speaker boundaries" (~line 108) governs token-timestamped rows and is amended so that token-less rows split proportionally at turn boundaries and the manual-row guard is scoped per the re-diarization requirement above (SUPERSEDED 2026-09-13 by no-split-sentences: token-less rows split proportionally at SENTENCE granularity, each sentence assigned whole — see "Sentences are not split across speaker badges"); "Re-transcription clears and re-enqueues diarization" (~line 491) is re-pointed to the `run_diarization_for_meeting` transport. At archive these requirement blocks are updated with this scoping, and the headline requirement is RENAMED to drop the stale "queue phase" phrasing.

### Requirement: Short-duration noise speakers are merged into nearest cluster

After clustering, speakers with total speech duration below `MIN_CLUSTER_FRAC × total_audio_secs` (default 2%) SHALL be merged into their cosine-nearest larger cluster. The absolute floor SHALL be `MIN_SPEECH_SECS` (1.5s) — the model's own minimum embedding input.

After merging, adjacent segments with the same speaker SHALL be coalesced, and speaker IDs SHALL be renumbered in temporal first-appearance order.

#### Scenario: Noise speakers merged in 3-speaker meeting

- **GIVEN** clustering produces 7 speakers where 4 have total duration < 3s each and 3 have > 100s each
- **WHEN** the short-speaker merge runs
- **THEN** the 4 short speakers are reassigned to their cosine-nearest large speaker
- **AND** the final output has exactly 3 speakers

---

### Requirement: Token-level timestamps align transcript text with diarization speaker boundaries

The diarization processor SHALL read its alignment input and token timestamps from the
`transcript_sources` table — the immutable per-meeting copy of the transcription rows —
never from the `transcripts` rendering rows it wrote on a previous run. The engine's
transcript-prior stage (`fetch_transcript_timestamps`, feeding turn derivation) SHALL
read from `transcript_sources` as well. When a Whisper segment spans multiple speakers,
the text SHALL be split at the speaker change boundary, producing separate rendering
rows per speaker.

When token timestamps are unavailable in the source rows (e.g., a legacy meeting whose
backfilled source predates the token-timestamps feature), the processor SHALL fall back
to segment-level timestamps with proportional text-split as a degraded alignment mode.

Persist is **full-meeting regeneration**, not in-place mutation: after alignment,
merge, and duplicate resolution, the system SHALL rebuild the meeting's auto rendering
in ONE transaction — (1) load the meeting's manually-corrected rendering rows
(`speaker_source = 'manual'`); (2) suppress any aligned segment whose midpoint falls
inside a surviving manual row's time span; (3) DELETE all rendering rows of the meeting
except the surviving manual rows; (4) INSERT every non-suppressed aligned segment as a
fresh-UUID row — split text, clamped timing, resolved `speaker_label`,
`speaker_source = 'auto'`, `duration` recomputed from its own timing,
`token_timestamps = NULL`, and every other column copied from its SOURCE row (joined
via the segment's source-row id); (5) if the aligned output is empty while the source
table is non-empty, abort the transaction (a degenerate run never wipes the rendering).
A scheme that mutates prior rendering rows by id lookup (last-writer-wins UPDATE, or a
delete-by-input-id sweep that treats previous output as absorbed) SHALL be considered
NON-CONFORMANT: input row ids identify SOURCE rows, and rendering row ids have no
stable relationship to them across runs.

Rows carrying `token_timestamps = NULL` in the rendering is expected and permanent —
rendering rows do not carry engine internals; the aligner no longer reads them. A
persisted row's `previous_label` SHALL be NULL (previous_label is rendering history
carried by manually-labeled rows, which survive regeneration untouched; fresh
regenerated rows have no label history).

A rendering row whose `speaker_source = 'manual'` SHALL survive regeneration untouched
(not deleted, not relabeled), preserving user corrections; aligned segments whose
MIDPOINT falls inside a surviving manual row's time span are suppressed rather than
duplicated. The midpoint predicate is the whole predicate: a segment straddling a
manual span is suppressed or kept in full according to its midpoint alone (its text
then renders whole under one badge — trade-off accepted; every production path
pre-clears labels before persist, so the manual set is defense-in-depth). On the
explicit re-derive path (`rederive_manual`), no row survives: all rendering rows
regenerate and names re-apply via stamped-embedding matching (existing behavior).

Re-diarization idempotency is RE-DERIVATION, not shape-freezing: every run aligns the
same immutable source, so the auto rendering is a pure function of (source rows,
diarization segments, manual overlay). Unlike the previous doctrine, a split MAY
re-expand or re-shape on a later run when the engine's segments change — the split
shape is derived, never accumulated. The prior rationale for this clause (split rows
carrying NULL tokens to freeze their shape against a decaying input) is superseded:
the input no longer decays.

Re-transcription is clean-slate for BOTH tables: the re-transcription path does
`DELETE FROM transcripts WHERE meeting_id=?` AND `DELETE FROM transcript_sources WHERE
meeting_id=?` before inserting fresh rows, regenerating the immutable source from the
new engine output. A future "soft" re-transcription that updated text without deleting
rows SHALL be treated as a contract violation of this requirement.

All persisted text SHALL be bound via sqlx parameterized placeholders; transcript text
(untrusted Whisper output, including SQL meta-characters and prompt-injection
payloads) SHALL be treated as opaque data and SHALL NOT be interpreted, so adversarial
content in the transcript survives verbatim.

Word content SHALL survive the pipeline: the multiset of words in the meeting's
rendering equals the multiset of words in the meeting's source rows, with exactly
three exemptions — text of duplicate-cluster absorbed members (already present via the
survivor, which writes its text once); punctuation-only ranges (dropped by design);
and audio claimed by a surviving manually-corrected rendering row (the manual row
renders its own text instead, and a suppressed straddling segment's out-of-span words
remain in source only). This generalizes the earlier per-row split guarantee (the
whitespace-joined split text equaled the source row's text) to the sentence-atom
pipeline, where words legitimately move between rows via reattribution.

Source rows with no internal whitespace (e.g. CJK text without spaces) SHALL be
divided across speakers by the proportional path (character ratio proportional to
time) rather than assigned 100% to one speaker because the word count is 1.

Oversized source rows and the SQLite host-parameter ceiling SHALL be handled: per-row
INSERT statements (chunked if N × columns would approach the ceiling) complete without
OOM or a "too many SQL variables" error, preserving all words.

When no diarization segment overlaps a source row's time range (the proportional-path
tail case), the system SHALL label the row's words "Unknown Speaker" and keep the
row's own timing; it SHALL NOT borrow a diarization speaker from a non-overlapping
segment or emit a row with `audio_start_time` > `audio_end_time`.

Diarization for a given `meeting_id` SHALL be mutually exclusive across all write
paths: at most one diarization pass runs at a time per meeting, so the persisted
rendering reflects a single consistent pass rather than an interleaving of two.

#### Scenario: Single-speaker source row persists as one fresh rendering row

- **GIVEN** a source row with `audio_start_time = 5.0`, `audio_end_time = 9.0`, and all
  token timestamps fall within diarization speaker "Speaker 0" (5.0–9.0)
- **WHEN** the diarization processor aligns and persists
- **THEN** the meeting's rendering holds exactly one row for this audio with
  `speaker_label = "Speaker 0"` and `speaker_source = 'auto'`
- **AND** the row id is freshly generated (row ids are not stable across runs and no
  consumer may depend on their stability)

#### Scenario: Multi-speaker source row splits at boundary

- **GIVEN** a source row spanning 5.0–9.0 whose token timestamps show words at
  [5.0, 5.2, 5.4, 7.3, 7.5, 7.7], and diarization shows "Speaker 0" at 5.0–7.1 and
  "Speaker 1" at 7.2–9.0
- **WHEN** the diarization processor aligns and persists
- **THEN** the rendering holds two rows: text from tokens 5.0–5.4 under "Speaker 0"
  (5.0–7.1) and text from tokens 7.3–7.7 under "Speaker 1" (7.2–9.0), each with a fresh
  id, recomputed duration, `token_timestamps = NULL`
- **AND** every other column equals the SOURCE row's values (copied from
  `transcript_sources`, not from any prior rendering row)

#### Scenario: Second consecutive run does not shrink or corrupt the rendering

- **GIVEN** a meeting whose source holds N rows and whose rendering holds the output of
  a previous Speakers run (fresh-UUID rows that share no ids with the source rows)
- **WHEN** the Speakers pipeline runs again with the same diarization segments
- **THEN** the rendering is rebuilt to the same row count and the same (text, span,
  badge) multiset as before the run
- **AND** no output row is deleted as "absorbed" and no aligned segment is discarded
  for lack of a rendering row with the source row's id

#### Scenario: Manually-corrected rendering row survives regeneration

- **GIVEN** a rendering row with `speaker_source = 'manual'` spanning 39.0–41.0 s
- **WHEN** regeneration persists a new Speakers run
- **THEN** the manual row is untouched (same id, text, label, `previous_label`)
- **AND** aligned segments whose MIDPOINT falls inside 39.0–41.0 are not inserted
  (no duplicate text); a segment straddling the span is suppressed or kept whole
  according to its midpoint alone
- **AND** segments with midpoints outside the manual span persist normally

#### Scenario: Degenerate alignment does not wipe the rendering

- **GIVEN** a meeting with a non-empty source table and prior rendering
- **WHEN** an alignment pass yields zero segments and persist is invoked
- **THEN** the transaction aborts: the prior rendering rows are untouched (their
  labels were already cleared earlier in the run's pre-clear step — the abort
  prevents row loss, not label clearing)

#### Scenario: Word content survives the pipeline (generalized all-words guarantee)

- **GIVEN** a meeting whose source holds rows totaling W alphanumeric words,
  including one row that alignment splits across two badges, one duplicate-cluster
  absorbed member, and no surviving manual rows
- **WHEN** the Speakers pipeline persists
- **THEN** the rendering's words equal the source's words minus the absorbed member's
  duplicate copy (present once via the survivor) and minus punctuation-only ranges
- **AND** the earlier per-row form still holds where applicable: for a source row that
  splits into rows under the same reattribution set, the whitespace-joined text of
  those rows equals the source row's text
- **AND** when a surviving manual row claims a span, the claimed span's source words
  are exempt (the manual row's own text renders; suppressed straddler text remains in
  source only)

#### Scenario: CJK / no-whitespace text is divided, not dumped to one speaker

- **GIVEN** a source row with no internal whitespace (e.g. CJK text without spaces)
  whose span overlaps two diarization speakers
- **WHEN** the proportional-path alignment runs
- **THEN** the text is divided across the two speakers (character ratio proportional
  to time), not assigned 100% to one speaker because `words.len() == 1`

#### Scenario: Oversized source row and SQLite host-param ceiling are handled

- **GIVEN** a source row whose text is ~500 kB, or a source whose alignment yields N
  segments such that N × columns approaches the SQLite host-parameter ceiling
- **WHEN** regeneration persists
- **THEN** the operation completes without OOM or a "too many SQL variables" error
  (inserts are per-row and chunked if the ceiling would be exceeded), preserving all
  words

#### Scenario: Re-diarization re-derives the split shape from source

- **GIVEN** a meeting whose previous run split one source row into two rendering rows,
  and a later run's engine segments assign the whole span to one speaker
- **WHEN** the Speakers pipeline runs again
- **THEN** the rendering holds ONE row for that audio (the split re-expanded) with the
  full source text — derived from the immutable source, not accumulated damage
- **AND** the source table is byte-identical before and after every run

#### Scenario: Re-transcription rewrites both tables

- **GIVEN** a meeting with rendering rows and source rows
- **WHEN** re-transcription completes
- **THEN** both `transcripts` and `transcript_sources` for the meeting hold only the
  fresh engine output (full punctuation, token timestamps), with no stale row surviving
  in either table

#### Scenario: Proportional tail with no overlapping diarization does not borrow a foreign speaker

- **GIVEN** a source row whose time range does not overlap any diarization segment
- **WHEN** the proportional-path alignment runs on that row
- **THEN** the row's words are labeled "Unknown Speaker" (not a diarization speaker
  from a non-overlapping segment)
- **AND** the persisted row's `audio_start_time` is less than or equal to its
  `audio_end_time` (no inverted-range row is emitted)

#### Scenario: Transaction atomicity — a failure mid-write leaves no partial regeneration

- **GIVEN** a regeneration whose rendering delete succeeds but an insert fails
- **WHEN** the persist transaction errors
- **THEN** the transaction rolls back and the meeting's prior rendering is fully intact

### Requirement: Centroid embeddings are stored per speaker per meeting for cross-meeting matching

The diarization processor SHALL return centroid embeddings. Centroids are duration-weighted averages of per-chunk embeddings, computed during agglomerative clustering **and refined by the temporal-coherence smoothing pass before storage** (see the temporal-coherence requirement below). The stored centroids SHALL be the post-smoothing recomputed values, not the pre-smoothing clustering centroids, so that cross-meeting matching operates on de-contaminated voice profiles. They SHALL be stored in the `speaker_embeddings` table as BLOBs with the cluster label and source meeting ID.

Embedding dimensions are model-dependent (not hardcoded). The storage layer SHALL accept any dimension in range [64, 1024] and validate that all values are finite.

When a user labels a speaker (e.g., "Speaker 0" → "Alice"), the system SHALL create or update a `speakers` table row with the name and persistent color, and link the corresponding `speaker_embeddings` row to the named speaker.

#### Scenario: Centroids stored after temporal-coherence refinement

- **WHEN** diarization identifies 3 speakers in a meeting
- **THEN** 3 rows are inserted into `speaker_embeddings`, each containing the duration-weighted centroid embedding for that cluster AFTER the temporal-coherence pass has recomputed it from cleaned labels, the source meeting ID, and a generated cluster label ("Speaker 0", "Speaker 1", "Speaker 2")

#### Scenario: Labeling a speaker creates a named profile

- **WHEN** the user labels "Speaker 0" as "Alice"
- **THEN** a row is inserted or updated in `speakers` with `name = "Alice"` and a persistent color from the palette
- **AND** the corresponding `speaker_embeddings` row is linked to the named speaker via `speaker_id`
- **AND** all transcript rows with `speaker_label = "Speaker 0"` in that meeting are updated to `speaker_label = "Alice"`

---

### Requirement: Cross-meeting speaker matching uses embedding similarity

After diarization assigns anonymous speaker labels ("Speaker 0", etc.), the system SHALL compare each speaker cluster's centroid embedding against all named speakers in the `speakers` table using cosine similarity. Matches above the threshold SHALL auto-label the speaker with the matched name.

The threshold SHALL default to 0.40 and SHALL be configurable via advanced settings in range [0.35, 0.70].

#### Scenario: Matching speaker auto-labeled

- **GIVEN** "Alice" exists in the `speakers` table with stored embeddings
- **WHEN** diarization produces a speaker cluster with centroid embedding cosine similarity ≥ 0.60 to Alice
- **THEN** the speaker is labeled "Alice" directly

#### Scenario: No match produces cluster label

- **GIVEN** no speakers in the registry have similarity ≥ threshold
- **WHEN** diarization produces a speaker cluster
- **THEN** the speaker keeps its cluster label ("Speaker 0")

---

### Requirement: Retroactive speaker labeling via inline badges with per-speaker revert

The frontend SHALL render an inline speaker badge next to each transcript segment. The badge SHALL display the current speaker label ("Speaker 0", "Alice", "Unknown Speaker"). Clicking the badge SHALL open an inline input to type a new name or select from existing named speakers.

When the user assigns a name, the frontend SHALL invoke `label_speaker(meeting_id, cluster_label, speaker_name)`, which creates/updates the `speakers` row, links embeddings, and updates all transcript rows for that cluster in the meeting. The original cluster label SHALL be preserved in a `previous_label` column on each transcript row (set only once, on first manual label).

The inline input SHALL show suggestion chips of existing named speakers (excluding auto-generated "Speaker N" labels). Selecting an existing speaker name SHALL merge the current cluster into that speaker — all transcript segments for the cluster are relabeled to the selected name. This is an intentional merge action, not a rename.

Manually-named speaker badges SHALL show a small undo icon (visible on hover) that reverts that speaker to its original auto-generated cluster label. Clicking the icon SHALL invoke `revert_speaker_label(meeting_id, speaker_label)`, which restores all transcript rows for that speaker in the meeting to their `previous_label`, sets `speaker_source` to `NULL`, and unlinks the corresponding embedding. The undo icon SHALL NOT appear on auto-generated labels ("Speaker N") or when `previous_label IS NULL`.

#### Scenario: Label an unknown speaker

- **WHEN** the user clicks the "Speaker 0" badge and types "Alice"
- **THEN** the badge updates to "Alice" with Alice's persistent color
- **AND** all transcript segments from "Speaker 0" in this meeting update to "Alice"

#### Scenario: Merge a cluster into an existing speaker via suggestion chip

- **GIVEN** a meeting where "Speaker 0" was renamed to "Alice" and "Speaker 1" was renamed to "Bob"
- **WHEN** the user clicks the "Speaker 2" badge and selects "Alice" from the suggestion chips
- **THEN** all transcript segments from "Speaker 2" are relabeled to "Alice"
- **AND** "Speaker 2" is effectively merged into "Alice" for this meeting

#### Scenario: Re-label a previously named speaker

- **WHEN** the user clicks the "Alice" badge and types "Bob"
- **THEN** the badge updates to "Bob"
- **AND** the `speakers` row for this cluster is updated to `name = "Bob"`
- **AND** all transcript segments in this meeting update to "Bob"
- **AND** the embedding previously linked to "Alice" is now linked to "Bob" for this meeting only — other meetings with "Alice" are unaffected

#### Scenario: Revert a named speaker to original cluster label

- **GIVEN** a meeting where the user manually renamed "Speaker 0" → "Alice"
- **WHEN** the user hovers over the "Alice" badge and clicks the undo icon
- **THEN** all transcript rows with `speaker_label = "Alice"` in that meeting revert to `speaker_label = "Speaker 0"`
- **AND** `speaker_source` is set to `NULL`
- **AND** `previous_label` is cleared to `NULL`
- **AND** the corresponding embedding is unlinked (`speaker_id = NULL`)

#### Scenario: Revert after merge restores different original labels

- **GIVEN** a meeting where "Speaker 0" was renamed to "Alice" and "Speaker 2" was also renamed to "Alice"
- **WHEN** the user reverts "Alice"
- **THEN** some transcript rows revert to "Speaker 0" and others revert to "Speaker 2" (each row has its own `previous_label`)
- **AND** the two original clusters are restored independently

#### Scenario: Revert disabled for auto-generated labels and legacy manual labels

- **GIVEN** a transcript segment with `speaker_label = "Speaker 0"` (auto-generated) or a manual label from before the `previous_label` migration (where `previous_label IS NULL`)
- **THEN** the undo icon is not shown on the badge

#### Scenario: Full reset clears previous_label

- **WHEN** the user triggers re-diarization (Speakers button)
- **THEN** all `previous_label` values are cleared along with `speaker_label` and `speaker_source`

---

### Requirement: Re-diarization cleans up stale state and resets speaker labels

When the user triggers "re-diarize" (Speakers button) on a meeting, the system SHALL perform a full reset:

1. Clear **all** speaker labels on transcript rows (both `"auto"` and `"manual"`)
2. Delete all embeddings in `speaker_embeddings` for that meeting (stale centroids from previous runs)
3. Delete auto-generated speaker rows (`speaker-auto-{meeting_id}-*`) from the `speakers` table
4. Re-run offline diarization on the full audio
5. Store fresh centroid embeddings from the new clustering
6. Match new speaker clusters against existing named speakers by embedding similarity

The system SHALL emit a `diarization-complete` event with the updated speaker assignments.

#### Scenario: Re-diarize resets all labels to fresh cluster labels

- **GIVEN** a meeting where the user manually corrected "Speaker 1" → "Bob" and "Speaker 0" → "Alice"
- **WHEN** the user triggers re-diarization (Speakers button)
- **THEN** ALL speaker labels are cleared (including "Bob" and "Alice")
- **AND** stale embeddings and auto-generated speaker rows are deleted
- **AND** diarization runs fresh, producing new "Speaker 0", "Speaker 1", etc. labels
- **AND** if embedding similarity matches a cluster to a known speaker (e.g., "Alice" exists in the registry from another meeting), that label is auto-applied

#### Scenario: Re-diarize re-labels auto-assigned rows

- **GIVEN** a meeting where "Speaker 0" was auto-assigned to "Alice" with `speaker_source = "auto"`
- **WHEN** the user triggers re-diarization
- **THEN** all labels are cleared and diarization runs fresh
- **AND** new clusters are labeled based on the fresh embedding match

---

### Requirement: Speaker model selection and download

The system SHALL use a single embedding model: `nemo_titanet` (NeMo Titanet Small EN VoxCeleb, ~40 MB). The pyannote segmentation model (~6 MB) SHALL be a required download. Both models SHALL be downloaded during onboarding Step 3 or on first use if skipped during onboarding.

No user-facing model selector SHALL exist. The embedding model is hardcoded; speaker count is controlled by the merge threshold and max_speakers settings, not by model choice.

Existing databases with a `speaker_embedding_model` column holding a legacy value (e.g., `3dspeaker`) SHALL be migrated to `nemo_titanet` on upgrade. The column is retained for backward compatibility but no longer read by the diarization code.

#### Scenario: Onboarding downloads required models

- **WHEN** the user reaches onboarding Step 3
- **THEN** the pyannote segmentation model and the nemo_titanet embedding model are downloaded alongside the transcription (Whisper) and summary (Gemma) models

#### Scenario: Model download failure is graceful

- **WHEN** the speaker model download fails during onboarding
- **THEN** onboarding completes normally
- **AND** the diarization phase is skipped for subsequent recordings until the model is downloaded
- **AND** a warning is logged

#### Scenario: Legacy model value migrated on upgrade

- **GIVEN** an existing database where `settings.speaker_embedding_model = '3dspeaker'`
- **WHEN** the migration runs on upgrade
- **THEN** the value is updated to `nemo_titanet`
- **AND** subsequent diarization jobs load the nemo_titanet model file

---

### Requirement: Per-speaker persistent colors

Each speaker in the `speakers` table SHALL have a `color` field assigned using golden-angle HSL distribution (`hue = index × 137.508 mod 360`, saturation 65%, lightness 55%) when the speaker is first created. The color SHALL be used consistently across all meetings where that speaker appears.

#### Scenario: New speaker gets a color from the palette

- **WHEN** the user labels a speaker for the first time
- **THEN** the speaker is assigned the next available color from the golden-angle palette
- **AND** all transcript segments for that speaker in all meetings display with that color

#### Scenario: Known speaker retains color across meetings

- **GIVEN** "Alice" has `color = "hsl(137, 65%, 55%)"`
- **WHEN** Alice is auto-matched in a new meeting
- **THEN** her badge and transcript segments use the same color

---

### Requirement: Merge threshold configurable in settings

The clustering merge threshold SHALL default to 0.40 and SHALL be configurable via settings in range [0.35, 0.70]. Higher values produce more speakers (more conservative merging). Lower values produce fewer speakers (more aggressive merging). The threshold controls the cosine similarity below which two clusters are merged.

#### Scenario: Default threshold produces correct speaker count

- **GIVEN** a meeting with 3 speakers
- **WHEN** diarization runs with threshold 0.40
- **THEN** 3 speakers are identified after short-speaker merge

#### Scenario: Higher threshold produces more speakers

- **GIVEN** a meeting with 3 speakers
- **WHEN** diarization runs with threshold 0.60
- **THEN** more than 3 speakers are identified (clusters stay separate)

---

### Requirement: max_speakers cap merges most isolated cluster

The effective max_speakers cap for a meeting SHALL be the meeting's per-meeting override (`meetings.max_speakers`) when it is set (NOT NULL), otherwise the global `settings.max_speakers` (default 10, range [2, 20]). When the cluster count after short-speaker merge exceeds the effective cap, the system SHALL reduce the count by repeatedly merging the most isolated cluster — the cluster with the lowest nearest-neighbour centroid cosine similarity — into its nearest neighbour. The cap is an upper bound, not a target: the system SHALL NOT split clusters and SHALL NOT merge clusters when the cluster count is at or below the effective cap. The system SHALL NOT merge the highest-similarity pair, as two real speakers who sound alike can have higher centroid similarity than a noise/outlier cluster, and merging them would destroy separation.

#### Scenario: Excess cluster absorbed without collapsing similar speakers

- **GIVEN** a meeting with 3 speakers where clustering at threshold 0.65 produces 4 clusters
- **AND** two real speakers have centroid sim 0.473 (highest pair)
- **AND** the noise cluster has nearest-neighbour sim 0.327 (lowest)
- **WHEN** the effective max_speakers for the meeting is 3
- **THEN** the noise cluster is merged into its nearest neighbour
- **AND** the two real speakers remain separate

#### Scenario: Per-meeting override takes precedence over global default

- **GIVEN** the global `settings.max_speakers` is 10
- **AND** a meeting has `meetings.max_speakers = 3` (per-meeting override)
- **WHEN** diarization runs on that meeting and produces 5 clusters
- **THEN** clusters are merged down to exactly 3 (the override), not 10 (the global default)

#### Scenario: NULL override falls back to global default

- **GIVEN** the global `settings.max_speakers` is 6
- **AND** a meeting has `meetings.max_speakers IS NULL`
- **WHEN** diarization runs on that meeting and produces 8 clusters
- **THEN** clusters are merged down to 6 (the global default)

#### Scenario: Effective cap above cluster count is a no-op

- **GIVEN** a meeting whose effective max_speakers is 5
- **WHEN** diarization produces 3 clusters
- **THEN** no merging occurs and the 3 clusters are preserved

#### Scenario: Degenerate centroid from garbled output is clamped, not propagated

- **GIVEN** diarization produces a cluster whose duration-weighted centroid is degenerate (contains NaN or Inf values — e.g. from a garbled, non-silent Whisper chunk whose ONNX embedding extraction numerically underflows or overflows; genuinely silent audio is rejected upstream by the embedding extractor's `is_effectively_silent` energy guard before any embedding is produced)
- **AND** the cluster count exceeds the effective max_speakers cap so the most-isolated-cluster merge runs
- **WHEN** the cap enforcement selects and merges clusters
- **THEN** the cosine similarity between a degenerate centroid and any other centroid SHALL be clamped to a finite 0.0 by two conjuncts acting together: the `norm > 0.0` guard (which catches NaN, since a NaN norm makes the `>` comparison false) AND the `dot.is_finite()` guard (which catches Inf, since an Inf centroid has an Inf norm that passes `norm > 0.0` and would otherwise yield Inf/Inf = NaN at the division) — both conjuncts are required, so the degenerate cluster ranks as most-isolated (0.0) rather than corrupting the isolation ranking with a NaN
- **AND** the degenerate cluster SHALL be absorbed into its nearest neighbour with both the survivor's and the absorbed centroid's values clamped to finite (a non-finite value contributes 0.0, not its non-finite geometry, so the survivor's centroid is not corrupted)
- **AND** every surviving centroid SHALL remain finite after the cap completes, so the degeneracy cannot cascade into the remaining clusters on subsequent merges nor reach the `speaker_embeddings` table (whose storage layer requires all values finite)

> **Scope:** This scenario governs the cap-enforcement path only (`cosine_similarity_centroids` in `commands.rs`, whose sole non-test caller is `enforce_max_speakers_cap`). The upstream clustering and short-speaker-merge paths use a separate similarity helper without the `dot.is_finite()` conjunct; their defense against non-finite values is `is_effectively_silent` (which rejects silence but not garbled non-silent audio that can still yield a non-finite ONNX output) plus the `speaker_embeddings` storage finite-check, which rejects non-finite values at persistence time.

### Requirement: Re-transcription clears and re-enqueues diarization

When a user re-transcribes a meeting with a different model, the system SHALL clear all speaker labels from the meeting's transcript rows and re-enqueue a `Diarizing` phase job for that meeting.

#### Scenario: Re-transcription triggers re-diarization

- **WHEN** the user re-transcribes a meeting that has speaker labels
- **THEN** all transcript rows for that meeting have `speaker_label` set to `NULL` and `speaker_source` set to `NULL`
- **AND** a diarization job is enqueued for the meeting

### Requirement: Per-meeting max_speakers override is configurable

Each meeting SHALL carry an optional max_speakers override stored as a nullable `meetings.max_speakers INTEGER` column. The override SHALL be settable and clearable via `set_meeting_max_speakers(meeting_id, cap)`, where `cap` is either an integer in [2, 20] or `None` (which clears the override to NULL). The system SHALL reject values outside [2, 20] and SHALL reject a `meeting_id` that does not exist in the `meetings` table. A `get_meeting_max_speakers(meeting_id)` query SHALL return the override value (or its absence), the effective cap (override if set, else the global default), and the global default, so the UI can render the current state in a single call.

The frontend SHALL surface the override in the meeting's speaker panel as a "Max speakers" control with an explicit "Auto (use default: N)" option that maps to NULL. Setting the override SHALL persist it immediately; the override SHALL take effect on the next diarization or re-diarization run for that meeting. The override control SHALL NOT trigger re-diarization automatically, because re-diarization clears all speaker labels including manual corrections.

#### Scenario: Set a per-meeting override

- **GIVEN** a meeting exists in the `meetings` table
- **WHEN** the user sets the meeting's max speakers to 3
- **THEN** `meetings.max_speakers` is stored as 3 for that meeting
- **AND** the next diarization run for that meeting uses 3 as the effective cap

#### Scenario: Clear the override to use the global default

- **GIVEN** a meeting with `meetings.max_speakers = 3`
- **WHEN** the user selects "Auto (use default)"
- **THEN** `meetings.max_speakers` is set to NULL
- **AND** the next diarization run uses the global `settings.max_speakers`

#### Scenario: Override is applied on re-diarization

- **GIVEN** a meeting already diarized with the global default (10) that produced 5 speakers
- **AND** the user sets the meeting's max speakers override to 3 and triggers re-diarization
- **THEN** re-diarization runs with effective cap 3
- **AND** the result has at most 3 speakers

#### Scenario: Out-of-range override rejected

- **WHEN** `set_meeting_max_speakers` is called with cap = 1 (or 21)
- **THEN** the call returns an error and `meetings.max_speakers` is left unchanged

#### Scenario: Non-existent meeting rejected

- **WHEN** `set_meeting_max_speakers` is called with a `meeting_id` not present in the `meetings` table
- **THEN** the call returns an error

### Requirement: Inline speaker-label input cancels on blur and preserves suggestion-chip submission

The inline `SpeakerLabelInput` SHALL cancel (dismiss without committing) when its text field loses focus, producing the same effect as pressing Escape; this requirement amends the "Retroactive speaker labeling via inline badges with per-speaker revert" requirement, which governs the open/submit/revert flow but is silent on dismiss mechanics. Cancelling on blur SHALL NOT dispatch `label_speaker`. Suggestion-chip buttons inside the input SHALL suppress the default focus shift on activation (via `preventDefault` on `mousedown`) so that selecting a suggested name submits the name via `onSubmit` rather than triggering blur-cancel and unmounting the input before the chip's click is delivered. Pressing Enter with non-empty text SHALL continue to submit, and pressing Escape SHALL continue to cancel. Pressing Tab (or any focus loss, including clicking a second speaker badge while one input is open) SHALL cancel, consistent with the click-outside semantics.

#### Scenario: Click outside cancels without committing

- **GIVEN** a transcript segment whose speaker badge has been clicked and the `SpeakerLabelInput` is open and focused
- **WHEN** the user clicks elsewhere in the document
- **THEN** the input is dismissed (unmounted)
- **AND** no `label_speaker` command is dispatched

#### Scenario: Typed name is discarded on click-outside

- **GIVEN** the `SpeakerLabelInput` is open with the text "Alice" typed into it
- **WHEN** the user clicks outside the input
- **THEN** the input is dismissed
- **AND** `label_speaker` is NOT dispatched (the typed name is discarded, not accidentally committed)

#### Scenario: Suggestion chip still submits after the blur guard

- **GIVEN** the `SpeakerLabelInput` is open, `knownSpeakers` is non-empty, and at least one suggestion chip matching the current typed text is visible
- **WHEN** the user clicks a visible suggestion chip
- **THEN** `label_speaker` IS dispatched with the clicked chip's name as `speakerName`
- **AND** the input is dismissed after the submit

#### Scenario: Keyboard paths are unchanged

- **GIVEN** the `SpeakerLabelInput` is open with non-empty text
- **WHEN** the user presses Enter
- **THEN** `label_speaker` is dispatched (submit) — unchanged from before this change
- **AND WHEN** the user presses Escape instead
- **THEN** the input is dismissed without dispatching `label_speaker` (cancel) — unchanged from before this change

#### Scenario: Tab and second-badge focus loss cancel (documented trade-off)

- **GIVEN** the `SpeakerLabelInput` is open with text typed into it
- **WHEN** the user presses Tab, or clicks a second speaker badge while the first input is open
- **THEN** the first input is dismissed (cancel) without dispatching `label_speaker`
- **AND** this is an intentional, documented trade-off: the input is a transient inline affordance, not a tab-stop in a form flow

### Requirement: Inline speaker-label input supports per-segment override in addition to cluster rename

The inline `SpeakerLabelInput` SHALL offer a scope control that lets the user choose whether a typed name applies to every segment in the current cluster (the existing cluster-rename behavior) or to the single transcript segment whose badge was clicked (a per-segment override); this amends the "Retroactive speaker labeling via inline badges with per-speaker revert" requirement by extending inline labeling from cluster-only to cluster-or-single-segment via the existing `set_segment_speaker` path. The scope control SHALL default to cluster-wide so that the pre-existing rename flow is preserved without regression.

When the user chooses per-segment scope and submits a name, the frontend SHALL invoke `set_segment_speaker(transcript_id, speaker_name)`, which updates exactly one `transcripts` row: it sets `speaker_label` to the submitted name, `speaker_source` to `'manual'`, and `previous_label` to the row's prior `speaker_label` only if `previous_label` was previously `NULL` (set-once). The per-segment override SHALL NOT relabel any other row in the meeting. Suggestion-chip selection SHALL respect the same scope control as typed-name submission.

The submitted name SHALL be persisted via `sqlx` parameterized binding (`?` placeholder), which is the SQL-injection defense; `sanitize_speaker_name` trims, length-checks, and strips HTML but does not itself reject injection strings.

#### Scenario: Default scope is cluster rename (no regression)

- **GIVEN** a transcript segment whose speaker badge has been clicked and the `SpeakerLabelInput` is open
- **WHEN** the user types a name and submits without changing the scope control
- **THEN** `label_speaker` is dispatched with the meeting id, the current cluster label, and the typed name
- **AND** `set_segment_speaker` is NOT dispatched
- **AND** every transcript row in the meeting sharing that cluster label is relabeled

#### Scenario: Per-segment scope overrides exactly one row

- **GIVEN** the `SpeakerLabelInput` is open for a segment whose cluster label is "Speaker 2"
- **WHEN** the user switches the scope control to per-segment and submits the name "Carlos"
- **THEN** `set_segment_speaker` is dispatched with that segment's `transcript_id` and speaker name "Carlos"
- **AND** `label_speaker` is NOT dispatched
- **AND** only that one transcript row is relabeled to "Carlos"; other "Speaker 2" rows in the meeting are unchanged

#### Scenario: Suggestion chip respects per-segment scope

- **GIVEN** the `SpeakerLabelInput` is open with the scope control set to per-segment, `knownSpeakers` is non-empty, and at least one matching suggestion chip is visible
- **WHEN** the user clicks a suggestion chip
- **THEN** `set_segment_speaker` (not `label_speaker`) is dispatched with the chip's name for that segment's `transcript_id`

#### Scenario: Per-segment override sets previous_label exactly once

- **GIVEN** a transcript row with `speaker_label = "Speaker 2"` and `previous_label IS NULL`
- **WHEN** the user applies a per-segment override to "Carlos"
- **THEN** the row's `speaker_label` becomes "Carlos", `speaker_source` becomes `'manual'`, and `previous_label` becomes "Speaker 2"
- **AND WHEN** the user later overrides the same row again to "Bob"
- **THEN** `previous_label` remains "Speaker 2" (set-once), so revert still restores the original cluster label

#### Scenario: Per-segment override is cleared by the re-diarize button (inherited behavior)

- **GIVEN** a transcript row that received a per-segment manual override to "Carlos" (`speaker_source = 'manual'`)
- **WHEN** the user clicks the "Speakers" re-diarize button (which calls `reset_speaker_labels` → `clear_all_speaker_labels`)
- **THEN** the override is cleared along with all other labels (auto and manual), as required by the canonical "Re-diarization cleans up stale state" requirement
- **AND** this change does not alter that behavior; it inherits it

#### Scenario: Speakers button timeout and error recovery

- **GIVEN** a transcript exists with speaker labels
- **WHEN** the user clicks the "Speakers" re-diarize button
- **THEN** the system initiates a full re-diarization as documented in "Re-diarization cleans up stale state and resets speaker labels"
- **AND** the UI disables the button (showing "Analyzing…") and prevents double-clicks to avoid queuing multiple jobs
- **AND** the system tracks completion via one of three paths:
  1. The `diarization-complete` event fires → success toast shown and button re-enabled
  2. The background diarization command completes normally → success toast shown and button re-enabled  
  3. Neither completes within 5 minutes → timeout error toast shown and button re-enabled (user can retry)
- **AND** per-meeting `diarization_lock` in the backend ensures only one diarization job runs at a time for the same meeting, preventing parallel execution
- **AND** if the UI fails to register the `diarization-complete` event listener, the diarization command is still executed and the UI recovers via the 5-minute timeout path

### Requirement: Speakers button timeout and error recovery

When the user clicks the "Speakers" re-diarize button, the system SHALL enforce:

1. **UI safety** – Button is immediately disabled and marked "Analyzing…" to prevent double-clicks; button is re-enabled when any completion path fires or after a 5-minute timeout, whichever comes first
2. **Completion tracking** – The system SHALL race three completion paths:
   - The `diarization-complete` event (primary success path)
   - Completion of the background diarization command (fallback path) 
   - A 5-minute timeout safety net (for backend crashes / IPC hangs)
3. **User feedback** – Success (via event or command completion) shows success toast; timeout shows error toast; both cases re-enable the button so the user can retry
4. **Backend constraints** – The per-meeting `diarization_lock` in `commands.rs` ensures only one diarization job runs at a time for the same meeting, preventing queue buildup or parallel execution
5. **Error resilience** – If the UI fails to listen for the `diarization-complete` event, the diarization command is still executed and the UI recovers via the 5-minute timeout

#### Scenario: Speakers button timeout (5 minutes)

- **GIVEN** a transcript exists and user clicks "Speakers" button
- **AND** the backend fails (dev server crash, IPC hang) and neither the `diarization-complete` event nor the command completion occurs within 5 minutes
- **THEN** the UI shows an error toast: "Re-diarization timed out – the diarization process may have failed. Please try again."
- **AND** the button is re-enabled so the user can retry the operation
- **AND** a log entry records the timeout for operational monitoring

#### Scenario: Speakers button timeout recovery

- **GIVEN** a previous "Speakers" button click timed out and user sees the error toast
- **WHEN** the user clicks the "Speakers" button again
- **THEN** the system enforces the same timeout and error recovery behavior as the initial click
- **AND** the new attempt is properly locked by the per-meeting `diarization_lock` if one is still running (if the first attempt's backend process is still running despite the timeout)

This requirement ensures that the Speakers button is resilient to backend failures, provides clear feedback to users, and prevents UI deadlock while respecting the backend's per-meeting execution constraints.

- **GIVEN** a transcript row overridden per-segment from "Speaker 2" to "Carlos", where the row had a non-null `previous_label`
- **WHEN** the user reverts "Carlos" via the existing badge undo (which calls `revert_speaker_label(meeting_id, "Carlos")`)
- **THEN** that row's `speaker_label` is restored to its own `previous_label` ("Speaker 2")
- **AND** any other rows in the meeting labeled "Carlos" are restored to their own respective `previous_label` values independently

#### Scenario: Known limitation — never-labeled row is not revertible

- **GIVEN** a transcript row with `speaker_label = NULL` and `previous_label IS NULL` (e.g., diarization was skipped)
- **WHEN** the user applies a per-segment override to "Carlos"
- **THEN** `previous_label` is set to the old `speaker_label` which is NULL, so it remains NULL
- **AND** a subsequent `revert_speaker_label` for "Carlos" does NOT restore that row (the `WHERE previous_label IS NOT NULL` guard excludes it), leaving a non-functional undo for that row — a documented limitation

#### Scenario: Hostile speaker name is bound as a parameter, not interpolated

- **WHEN** `set_segment_speaker` is called with a name containing SQL-injection content (e.g., `'; DROP TABLE transcripts; --`)
- **THEN** the name is bound via a `sqlx` `?` placeholder (parameterized query), so it is treated as a literal value
- **AND** no transcript row is modified beyond the targeted id and no table is affected

#### Scenario: Non-existent transcript_id is a safe no-op

- **WHEN** `set_segment_speaker` is called with a `transcript_id` that does not exist in the `transcripts` table
- **THEN** the command returns `Ok(false)` (0 rows affected)
- **AND** no error is raised and no row is mutated

### Requirement: Temporal-coherence smoothing prevents clustering contamination and per-chunk flicker

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
> mis-assigned. On `meeting-00000001-…` one of three speakers is absorbed from minute ~30 onward
> under both the production global AHC and a sequential online-centroid-tracking prototype. A
> read-only diagnostic (`test_00000001_embedding_drift_diagnostic`) **ruled out** the
> embedding-drift hypothesis originally suspected — the absorbed speaker's OWN late chunks are
> cos ≈ 0.85 to her early centroid (same-speaker range), NOT the ≈ 0.22 figure cited earlier
> (which was the mean cosine of ALL late chunks to her centroid, low only because most late
> chunks belong to other speakers). The root cause is not yet determined and is filed as a
> separate change; do NOT re-attempt a label-level fix for sustained absorption without first
> establishing the cause.

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

### Requirement: Diarization segment granularity resolves speaker turns within Whisper segments

Whisper groups transcript segments by sentence/VAD, not by speaker; on multi-speaker meetings these segments routinely span 15–30s and contain two or more speakers. The diarization output SHALL be granular enough that a speaker turn occurring inside a single Whisper transcript segment produces a diarization segment boundary at or near the turn, so that per-word alignment can attribute the words on each side of the turn to the correct speakers rather than collapsing the whole segment to one speaker.

Speaker change-point boundaries SHALL be sourced from a pyannote `ort::Session` running **in-process** — its own `ort::Session` over `pyannote-segmentation-3.0`, alongside the nemo_titanet embedding session, the exact pattern the Phase 1 probe (`pyannote_ort_probe.rs:48-59`) validated. The segmentation + sliding-window + powerset-decode + smoothing + boundary-emission logic is the productionized form of the Phase 2b probe (`pyannote_ort_probe.rs`): slide a 10s window at 1s step over the recording's 16 kHz mono samples, decode per-frame powerset logits to 3-speaker multilabel activity via hysteresis at onset 0.5, apply pyannote-default smoothing (median filter rad=3, min_on=0.3s, max_off=0.5s — the only Phase 2b config that hit BOTH known anchors), and emit `Vec<(start_seconds, end_seconds)>` change-points. The diarization flow (`commands.rs:413-432`) SHALL INTERSECT the pyannote change-points with the Whisper `transcript_segments` (`fetch_transcript_timestamps`) — a pyannote change-point inside a Whisper speech region is kept as an intra-region split; the Whisper silence regions are preserved as silence (not embedded). The intersected set is passed as the `transcript_segments` argument to `adapter.process()`.

**Why in-process on one runtime, not a subprocess:** sherpa-onnx-sys 1.13.4 statically bundles ORT 1.17.1 (C-API ≤17); the project's `ort = "2.0.0-rc.10"` dep (the app's ONNX inference dependency) brings C-API 27. The two runtimes collide on the global C-API symbol table the moment both are linked into one process → STATUS_ACCESS_VIOLATION. This was verified by the `pyannote_sherpa_load_crux` probe. This change resolves the conflict at the root by **removing sherpa-onnx entirely** and porting nemo_titanet embedding extraction to the `ort` crate (see design.md D1); with sherpa gone, nemo_titanet + pyannote share one ORT runtime — no conflict by construction. The port is empirically validated: the `embed-probe-ort` crate reproduces sherpa's nemo_titanet embeddings at cosine **0.9946–0.9989** on production-relevant clips (clean/overlap ≥1.5s, non-silent) after a one-line log-floor fix (`f32::MIN_POSITIVE` → `f32::EPSILON`), well within the AHC operating margin. See `openspec/exploration/diarization-pyannote-boundaries-ort-probe.md` §"ARCHITECTURE LOOP CLOSED". A subprocess/IPC/second-binary path (Option 3) was panel-rejected as permanent subprocess debt once the port proved viable.

The pyannote boundary set AUGMENTS the Whisper transcript segments with intra-region splits; it does NOT supersede or replace the Whisper boundaries (which remain the speech-vs-silence mask). After this change, `build_chunks` sub-divides each Whisper speech region by the pyannote boundaries inside it and no longer applies `effective_split` as a boundary SOURCE (`sherpa_adapter.rs`); the only residual use of `effective_split` is the size guard that sub-divides surviving segments longer than `MAX_CHUNK_SECS`. The `MAX_DIARIZATION_CHUNKS` cap is enforced once, at the pyannote-boundary layer (see the uniform-shed scenario below). (A proposal that leaves both the uniform-grid step and the pyannote pre-splitter mandated as boundary sources simultaneously is NON-CONFORMANT — the canonical spec would contradict itself.)

The pyannote `ort::Session` emits boundaries only — no speaker labels, no embeddings (the session is over pyannote-segmentation-3.0 only). This is stronger than "labels discarded": there is nothing to discard. Meetily's AHC clustering, label-quality refinement, most-isolated-cluster cap, temporal-coherence smoothing, and cross-meeting registry matching remain authoritative for labeling, exactly as today.

#### Scenario: Sub-turn interjection is isolated, not swallowed

- **GIVEN** a Whisper transcript segment from 46:58 to 47:21 containing a 2s Ricardo interjection at 46:58–47:00 followed by Cynthia's speech
- **AND** the production diarization previously labeled the entire 46:58–47:30 run as Cynthia
- **WHEN** diarization runs with the in-process pyannote boundary source
- **THEN** the diarization output contains a speaker segment boundary near 47:00 separating Ricardo (≈46:50–47:00) from Cynthia (≈47:00 onward), so the interjection's words are attributed to Ricardo
- **AND** the chunk-grid-only baseline over the same window does not produce that boundary

#### Scenario: Back-and-forth between two speakers is not collapsed to one

- **GIVEN** a region where two speakers alternate in 4–8s turns across a 30s window
- **WHEN** diarization runs with the in-process pyannote boundary source
- **THEN** the output preserves the alternation as multiple segments rather than merging the window into a single speaker's run

#### Scenario: Single-speaker meeting is not fragmented

- **GIVEN** a meeting with exactly one speaker
- **WHEN** diarization runs with the in-process pyannote boundary source
- **THEN** the output is a single speaker (no spurious second cluster introduced by the finer boundary placement)

#### Scenario: Pyannote-model-missing falls back to the effective-split grid

- **GIVEN** the pyannote segmentation model file is absent from disk (not downloaded, or deleted)
- **WHEN** diarization runs and the in-process pyannote session cannot be constructed
- **THEN** the diarization proceeds with the canonical effective-split (`SPLIT_TARGET_SECS`) grid as the `transcript_segments` subdivision source
- **AND** the meeting still diarizes (at coarse resolution); only the finer pyannote boundaries are lost
- **AND** no panic propagates to the user-facing diarization flow

#### Scenario: Uniform shed-to-cap still recovers alternation turns on long meetings

- **GIVEN** a long (≥45 min) meeting with a rapid two-speaker alternation region and a single-speaker monologue region of comparable length
- **WHEN** the candidate-boundary count exceeds `MAX_DIARIZATION_CHUNKS` and uniform shedding runs (every k-th by position), followed by Meetily's AHC + temporal-coherence smoothing
- **THEN** the alternation region's turn structure is recovered (a threshold fraction of within-region turns are preserved in the final labeling), because turns are re-derived from the surviving candidate set, not carried by individual shed boundaries
- **AND** the resulting segment count after shedding is at or below `MAX_DIARIZATION_CHUNKS`

#### Scenario: Silent or empty audio does not crash the in-process flow

- **GIVEN** a silent or empty audio fixture
- **WHEN** the in-process pyannote session runs and yields an empty boundary set
- **THEN** the diarization proceeds (with an empty intersected set or the effective-split fallback) without panicking

#### Scenario: Corrupt-but-present pyannote model falls back to the effective-split grid

- **GIVEN** the pyannote segmentation model file is PRESENT on disk but corrupt (truncated, bad magic, or yields non-finite output mid-decode)
- **WHEN** the in-process pyannote `ort::Session` construction errors OR inference produces NaN/Inf
- **THEN** the diarization falls back to the canonical effective-split grid (the same fallback path as model-missing)
- **AND** the meeting still diarizes at coarse resolution (≥1 labeled `SpeakerSegment`)
- **AND** no panic propagates to the user-facing diarization flow

#### Scenario: A pyannote change-point exactly on a Whisper segment edge produces no zero-length split

- **GIVEN** a pyannote change-point whose timestamp coincides exactly with a Whisper `transcript_segment` start or end
- **WHEN** the intersect step runs
- **THEN** no zero-length split is emitted (the intersect SHALL deduplicate/clamp so every intra-region split has positive duration ≥ `MIN_SPEECH_SECS`, or is dropped)
- **AND** no `Chunk` with `duration_secs < MIN_SPEECH_SECS` reaches `adapter.process()`

#### Scenario: ort::Session wrapping preserves Send+Sync and clustering runs off the async executor

- **GIVEN** `ort::Session` is `Send + Sync` (ort 2.0.0-rc.10) and the port wraps it in `Mutex<Session>` (design D1) or a session-pool fallback
- **WHEN** the diarization `process()` runs
- **THEN** the wrapping remains `Send + Sync` so extraction + clustering execute on a blocking thread (per the canonical "Clustering does not freeze the UI" requirement), NOT on the async executor
- **AND** the async runtime and UI remain responsive during the diarization pass

#### Scenario: Concurrent multi-meeting diarization is isolated

- **GIVEN** N (≥2) meetings diarized concurrently, sharing the process's ort sessions (nemo_titanet + pyannote)
- **WHEN** their diarization passes interleave on the shared sessions
- **THEN** each meeting produces correct per-meeting results with no cross-meeting state leakage
- **AND** the shared-session contract is documented: either meetings serialize on the `Mutex<Session>` lock (no extraction interleaving across meetings) or each meeting gets an isolated session clone (memory cost)
- **AND** the shared registry (`HashMap<String, Vec<Vec<f32>>>`) does not corrupt under concurrent append (no panic, no wrong-label bleed across meetings)

### Requirement: Short chunks are not attributed to temporally-absent speakers

A diarization chunk whose duration is below a minimum presence threshold SHALL NOT retain a speaker label that has no other temporal support in the surrounding neighborhood. Such a chunk SHALL be relabeled to the temporally-dominant local speaker. This prevents short, vowel-dominated embeddings from being globally assigned to a speaker who has not yet appeared (or has long since left) the meeting.

A chunk that is short but lies between two genuinely different speakers (a real interjection) is a legitimate turn and SHALL be preserved — only chunks whose assigned label is a temporal orphan are relabeled.

#### Scenario: Opening utterance is not attributed to a speaker who has not joined

- **GIVEN** a meeting where Speaker 2 (Ricardo) first appears at 17:37
- **AND** a 1.4s chunk at 0:01 ("Hello") whose raw embedding is globally nearest to Ricardo's centroid
- **WHEN** the temporal-presence constraint is applied
- **THEN** the 0:01 chunk is relabeled to a speaker present at the start of the meeting (not Ricardo), because Ricardo has no temporal support near 0:01

#### Scenario: Genuine short interjection with nearby support is preserved

- **GIVEN** a 1.5s chunk labeled Ricardo (below `MIN_PRESENCE_SECS`, so the orphan scan does evaluate it), sandwiched between a Cynthia segment (left) and a Carlos segment (right)
- **AND** Ricardo has at least one other segment within `PRESENCE_WINDOW_SECS` on either side (Ricardo is a temporally-present speaker, not an orphan)
- **WHEN** the temporal-presence constraint is applied
- **THEN** the chunk retains the Ricardo label — the constraint relabels only orphans whose label has no nearby same-label support, and this chunk has support

---

### Requirement: The pyannote segmentation model is actually consumed by the in-process ort::Session

The pyannote-segmentation ONNX model SHALL be loaded and run by an in-process `ort::Session` (alongside the nemo_titanet embedding session) — NOT by a child binary or sherpa's `OfflineSpeakerDiarization` (which is non-viable due to the ORT runtime conflict). The session SHALL emit a deterministic, non-empty boundary set on real multi-speaker audio when the model is present, and SHALL change behavior (construction error or distinct/empty segmentation output) when the model file is swapped for a committed dummy fixture. This closes the prior phantom-dependency state where `segmentation_model_path` was accepted by the adapter constructor, existence-checked, and discarded.

#### Scenario: The in-process session loads and runs the segmentation model

- **GIVEN** the in-process pyannote `ort::Session` is constructed with a `model_dir` pointing at the on-disk pyannote model
- **WHEN** the session runs inference on a real multi-speaker clip
- **THEN** the emitted boundary set is deterministic and non-empty
- **AND** swapping the model file for a committed dummy fixture changes the session's behavior (construction error or distinct segmentation output — presence-of-path alone is not sufficient evidence of consumption)

### Requirement: nemo_titanet embedding extraction is ported to ort and sherpa-onnx is removed

The nemo_titanet embedding extraction SHALL be performed by an in-process `ort::Session` (the ported `NemoEmbeddingExtractor`, lifting the validated `embed-probe-ort` fbank + CMVN + pad-16 + transpose + session-builder pipeline) — NOT by sherpa-onnx's `SpeakerEmbeddingExtractor`. The `SpeakerEmbeddingManager` SHALL be replaced by a pure-Rust in-memory cosine store (`HashMap<String, Vec<Vec<f32>>>` + cosine search; sherpa's manager was a convenience wrapper, not a model). The `search` operation SHALL be a per-vector best-score scan — iterate every stored vector across all names and return the name of the single highest-cosine vector ≥ threshold — matching sherpa's `SpeakerEmbeddingManager::search` semantics exactly, NOT a per-speaker-centroid search (a centroid search would diverge when a speaker has one near-query vector and one far vector; the per-vector scan lets the near vector win). sherpa-onnx and sherpa-onnx-sys SHALL be removed from `Cargo.toml`, so the whole app links exactly one ORT runtime (the `ort` crate) and the C-API 17-vs-27 collision that motivated this change cannot occur by construction. The stored `speaker_embeddings` vectors remain nemo_titanet 192-dim — no schema migration. Registry hydration (`database/setup.rs`) SHALL construct the store at `dim = 192` (or read `dim()` from the extractor) — NOT the hardcoded `dim = 256` that silently loads zero speakers today (pre-existing bug fixed by this change).

#### Scenario: sherpa-onnx is no longer in the production dependency graph

- **GIVEN** the port is complete and `Cargo.toml` no longer declares `sherpa-onnx`
- **WHEN** `cargo tree -p meetily-flash` is run (scoped to the `meetily-flash` crate — NOT workspace root, because `embed-probe-sherpa` remains a workspace member as the cosine-gate reference binary, so workspace-root `cargo tree` still transitively shows sherpa)
- **THEN** neither `sherpa-onnx` nor `sherpa-onnx-sys` appears in the `meetily-flash` dependency graph
- **AND** a grep for `sherpa_onnx::` AND `SherpaOnnx` across BOTH `frontend/src-tauri/src/` AND `frontend/src-tauri/tests/` returns zero hits (the port replaced every sherpa reference in the speaker module, commands, state, database setup, smoke test, and the integration/probe tests)

#### Scenario: The port reproduces sherpa's embeddings within the AHC operating margin

- **GIVEN** the fixed 10-clip gate set plus production-representative additions, each clip ≥ 1.5s and passing `is_effectively_silent`, INCLUDING ≥4 clips uniformly distributed in [1.5, 3.0]s (the production pyannote-chunk regime) AND ≥2 clips at exactly 2.0s (the `refine_pass2` / `FINE_SPLIT_SECS` re-embedding window) — regimes that reach clustering, NOT dropped inputs
- **WHEN** the ported `NemoEmbeddingExtractor` and the sherpa reference extract embeddings from the same 16kHz mono clip
- **THEN** the cosine similarity between the two embeddings meets the margin-derived tiered threshold: ≥ 0.99 for clips ≥ 2.0s and ≥ 0.98 for clips in [1.5, 2.0)s — the floors are derived from the AHC separation margin (merge 0.40, inter-speaker cosine 0.6–0.8; measured residual worst-case 0.0131 is ~46× below the 0.60 inter-speaker floor), and SHALL be revised ONLY if that downstream margin changes — never in response to a failing measurement
- **AND** the per-clip cosine is reported (not just an aggregate pass/fail), so a regression in the 1.5–3s or 2.0s regime is visible
- **AND** the gate is re-run in full on any ORT-kernel upgrade (the drift-tripwire role — the bar guards future drift; AHC parity certifies the current port)
- **AND** before the gate is final: (a) ≥10 diverse-speaker 1.5s clips pass with worst-case ≥ 0.98 (tail evidence); (b) noise-injection invariance — reference embeddings perturbed by the measured worst-case residual (0.013) yield identical AHC clusterings
- **AND** filter parity holds — the port drops (via `is_effectively_silent`, `is_ready` / the minimum-frame gate, and `MIN_SPEECH_SECS`) exactly the clips sherpa drops, verified on a 25ms→2s sweep (not just the known cases)

**Speaker-attributed segment overlap (the parity metric).** For a reference labeling `ref` and a new labeling `new` over the same recording, for each speaker label `L` present in `ref`: `overlap(L) = |ref_segments(L) ∩ new_segments_same_speaker(L)| / |ref_segments(L)|`, where `ref_segments(L)` is the set of reference segments labeled `L` measured in seconds of audio, `new_segments_same_speaker(L)` is the set of new-run segments labeled with `L`'s corresponding label (labels matched across the two runs by Hungarian assignment on per-label segment-time overlap, to handle renumbering), and `∩` is temporal intersection in seconds. The score is the unweighted mean of `overlap(L)` over all labels `L` in `ref`. The per-label `overlap(L)` SHALL be reported (not just the mean), so a single collapsed speaker is visible rather than hidden in an aggregate.

#### Scenario: Extractor-only parity vs the sherpa reference (load-bearing)

- **GIVEN** committed multi-speaker fixtures
- **WHEN** the diarization runs TWICE with the SAME boundary source (`effective_split` grid — the pre-boundary-change chunk layout) but DIFFERENT extractors: once with the ported `NemoEmbeddingExtractor`, once with the sherpa extractor
- **THEN** the resulting cluster counts are identical
- **AND** speaker-attributed segment overlap (per the metric above) is ≥ 0.95, reported per-label
- **AND** this gate runs UNCONDITIONALLY on committed fixtures (NOT `#[ignore]`) — it isolates the extractor port (the change cosine was always a proxy for) from the boundary change

#### Scenario: Boundary-acceptance parity (confirmation)

- **GIVEN** ≥10 labeled multi-speaker recordings AND the pyannote model present
- **WHEN** the diarization runs TWICE with the SAME (ported) extractor but DIFFERENT boundary sources: once with pyannote boundaries, once with the `effective_split` grid
- **THEN** the resulting cluster counts are identical (the boundary change re-segments but AHC + smoothing recover the same speakers)
- **AND** speaker-attributed segment overlap (per the metric above) is ≥ 0.95, reported per-label (pyannote boundaries should match or improve overlap, not regress it)

### Requirement: Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits

On the success path (pyannote segmentation model present), the system SHALL derive speaker turns in two layers:

1. **Speech runs**: slide the production 10s/1s pyannote window grid over the full meeting (the final <1s tail SHALL be decoded with zero-padding so trailing speech is not silently dropped) and decode per-frame speaker-probability masses from the powerset output; collapse the speech-vs-silence track (speech = summed speaker mass > 0.5) into runs with minimum duration 0.3s. Min-duration collapsing SHALL absorb silence runs and same-label fragments only; a short run with a DIFFERENT label than its neighbor SHALL be retained as a piece and resolved by the attachment rules below.
2. **Sub-run splits**: a run containing a label-track change is split at the change point only where the change is corroborated: the two windows adjacent to the split time, each decoded independently, MUST each show a label-change event at the same time (agreement on the change EVENT and its timestamp within a ±0.35s tolerance — calibrated: per-window decodes jitter more than 0.2s at real changes — window-local index identities are permutation-ambiguous and are never compared across windows). Windows whose decodes disagree at the split point are treated as seam permutations and rejected; the run stays whole. A mode filter over the label track (not the per-speaker bool median filter, which does not apply to argmax labels; radius 3 frames) removes single-frame label flicker before split detection.
3. **Labeling**: each piece of duration ≥1.5s (the `MIN_SPEECH_SECS` floor; the model's minimum embedding input) SHALL be embedded — pieces longer than 12s embed their middle 12s, matching the validated measurement — and clustered by threshold clustering at the configured merge threshold with deterministic tie-breaking. Cluster count SHALL be capped by the meeting's max_speakers cap using the production most-isolated-cluster merge policy (`enforce_max_speakers_cap` semantics), and a nearest-centroid refinement pass SHALL reassign every labeled piece to its final centroids. Every persisted centroid SHALL correspond to at least one persisted labeled piece (no phantom speakers).
4. **Attachment**: a piece below the 1.5s floor MAY be embedded solely for attachment and SHALL attach to the temporally PREVIOUS labeled piece's turn (the following turn only when no previous exists); a piece whose final-centroid similarity margin between best and second-best is below a positive ambiguity margin (default 0.05, computed after the refinement pass) SHALL attach backward the same way. Attachment SHALL never create a new label, and contiguous backward-attached material exceeding 5s within one turn SHALL instead form its own turn flagged low-confidence in the data. Sub-floor arbitration is fixture-calibrated (design D3 amendment): a sub-floor piece at/above a promotion floor (0.8s; shorter slices win on noise) with a decided best cluster (margin ≥ the ambiguity margin) forms its own low-confidence turn; a sub-floor piece whose neighbors AGREE on one cluster joins them (burst fragmentation of one speaker's speech); a sub-floor piece sandwiched between two DIFFERENT clusters keeps the boundary as its own low-confidence turn with a best-effort label — a real interjection the 1.5s floor must not silently absorb.
5. **Textless runs**: runs with no transcript-text overlap (breaths, laughs, untranscribed voiced noise; detected with a whisper-timestamp skew tolerance) SHALL be dropped BEFORE same-cluster coalescing, so a textless run can never split one speaker's stretch into fragments.
6. **Coalescing and text alignment**: same-cluster neighbors separated only by dropped runs or absorbed silence SHALL coalesce, and the engine's turns ARE the persisted turn units on the success path — post-persistence turn merging layers (legacy consolidation, persist-path assembly) SHALL NOT re-run over engine output; where such a merge ever applies, `continues_previous` is re-derived from the merged members (first member's flag). Every transcript text row SHALL land in exactly one turn. Transcript text SHALL be divided at turn boundaries only at SENTENCE granularity (per "Token-level timestamps align transcript text with diarization speaker boundaries", as amended): each sentence is assigned whole to the turn owning the majority of the sentence's span — a reattribution of minority-span words to the sentence's badge; the engine's turn boundaries themselves are unchanged, and a turn whose span holds no sentence assigned to it emits no row. A row with zero time overlap with every turn (possible only in a shed span) SHALL attach to the nearest-in-time turn within the borrow cap; beyond the cap it stays "Unknown Speaker" (honest badge over confident misattribution). This content-preservation invariant SHALL be asserted end-to-end (each sentence's alphanumeric content appears in exactly one output turn).
7. **Overlap flags**: a turn's crosstalk flag SHALL be the fraction of its FINAL merged span whose per-frame overlap-pair probability mass (sum of powerset classes 4–6) exceeds 0.25, recomputed after all merging; fragment-maximum aggregation is forbidden. Overlap flags are persisted but not rendered by this change.
8. **Determinism**: all label decisions SHALL be computed from time-ordered or index-ordered sequences (ordered containers; no HashMap-iteration-order effect on labels, boundaries, or flags), so identical audio, models, and settings on the same binary produce identical turns. The clustering step itself SHALL be covered by a CI-runnable determinism unit test on synthetic embeddings.

Where this requirement and the chunk-grid labeling requirements conflict on the success path, this requirement governs; the pyannote-model-missing fallback path is unchanged. The number of embedded pieces per meeting SHALL be bounded by a shed-to-cap applied to runs before embedding (cap 2000, shed by position, merging sub-floor survivors within same-label spans of their speech region — never across a corroborated voice change), so clustering cost is bounded regardless of meeting length. Shedding is permanent on this path (no pass-2 re-labeling exists); spans lost to the cap lose attribution. If the embedding model fails to load, the run SHALL fail with the error surfaced (no partial labels); if pyannote inference fails mid-pass, the run SHALL fail without persisting partial labels. The label-track mode filter SHALL use radius 3 frames (≈50ms), calibratable only under the fixture-gate rule.

#### Scenario: Cynthia's attested sentence stays one voice until the attested 12.0 s change

- **GIVEN** the cde5c264 fixture entries that 9.38–11.8s is one voice (Cynthia's sentence, re-pinned 2026-09-07 via clip_D) and that the voice change to the user's "Yeah" sits at ≈12.0s (±0.75s)
- **WHEN** the engine derives turns
- **THEN** no turn boundary exists between 9.38s and 11.8s
- **AND** the turn change lands within the pinned tolerance of 12.0s — recovered by the embedding voice-flip check at the decode's confusion valley (change `engine-boundary-and-identity-accuracy`), asserted as a fully asserted gate entry with no waiver

#### Scenario: Voice change inside a run splits at the verified change point

- **GIVEN** the cde5c264 fixture entry of one voice change in 02:12–02:50s located at ≈161s (±0.75s per the fixture pin, user-ear-recorded 2026-09-04)
- **WHEN** the engine derives turns
- **THEN** the run splits at the corroborated change point, not at the ≈159s pause
- **AND** the sentence tail ("And I was like, oh, when you put a that one") is attributed to the earlier speaker
- **AND** a piece whose final-centroid margin is below the ambiguity margin attaches to the previous turn instead of opening a new one

#### Scenario: A window-seam permutation is rejected as a split

- **GIVEN** a synthetic two-window fixture where the per-frame label indices permute across the 1s window seam but each window's local track is internally consistent
- **WHEN** split candidates are evaluated
- **THEN** the seam-time change is rejected (adjacent-window corroboration fails) and the run stays whole

#### Scenario: Textless runs cannot dice a turn

- **GIVEN** a speaker's continuous speech containing sub-second voiced runs that whisper never transcribed
- **WHEN** turns are assembled
- **THEN** the textless runs are dropped before coalescing
- **AND** the surrounding same-speaker text renders as ONE turn (the "Where | is Ricardo" regression)

#### Scenario: Every transcript row survives into the output at sentence granularity

- **GIVEN** any diarization run on a meeting with transcript rows
- **WHEN** turns are assembled and persisted
- **THEN** each sentence's alphanumeric content appears in exactly one output turn
- **AND** a row straddling a turn boundary divides at sentence granularity — whole sentences to their majority-badge turn; no output row presents a fragment of another badge's sentence; a zero-overlap row attaches to the nearest-in-time turn within the borrow cap, else stays "Unknown Speaker"

#### Scenario: Corroborated text tail lands on the earlier turn

- **GIVEN** the 02:12–02:50 fixture entry, where the "And I was like, oh, when you put a that one" tail shares a whisper row with later-speaker text
- **WHEN** the rows are aligned to the derived turns
- **THEN** the row divides at sentence granularity at the ≈161s boundary and the tail sentence is assigned to the earlier turn (whole-row or word-split assignment would make this entry structurally unpassable)

#### Scenario: Overlap flag is span-truthful

- **GIVEN** a merged turn whose final span has raw-frame overlap mass above 0.25 on ~2.5% of frames
- **WHEN** the crosstalk flag is computed
- **THEN** the flag reports the span fraction within ±2 percentage points, not the maximum of any pre-merge fragment

#### Scenario: Edge regimes are handled

- **GIVEN** a meeting whose final second contains speech, and a meeting with no embeddable speech runs at all
- **WHEN** the engine runs
- **THEN** trailing speech receives frames via zero-padded final-window decode
- **AND** a meeting with zero labeled pieces produces zero speaker labels and no error

#### Scenario: Deterministic derivation

- **WHEN** the clustering and assembly functions run twice on identical inputs
- **THEN** labels, boundaries, and flags are identical (CI-runnable unit test on synthetic data)
- **AND** all tie-breaks resolve by index/time order, never by hash-map iteration order

### Requirement: Ear-truth fixture gate validates attribution

The repository SHALL contain a pinned ear-truth fixture (`frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json`) holding attribution facts as data, each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). The 13 recorded entries (user-ear answers of 2026-09-04, verbatim in this change's `fixture-answers.md`, resolved to absolute times via recovered clip offsets): single-voice spans ≈5.9–12.8 (user's sentence), 15.5–20.8, 24.5–29.5 (Cynthia), 32.0–38.0, 2803–2820 (Ricardo); voice changes at ≈13.0 (user→Cynthia), ≈29.5 (Cynthia→user), ≈31.5 (user→Cynthia "Yeah"), ≈38.0/≈39.0 ("okay" interjection), ≈2776.4 (two voices trading), ≈2803.0 (Cynthia→Ricardo), ≈2821.0 (Ricardo→Cynthia), and the 02:12–02:50s anchor `voice_change_at` ≈161s ±0.75 with the "And I was like, oh, when you put a that one" tail on the earlier side. Entries change only with explicit user confirmation, and two entries (`S3_updates_run`, `S13_ricardo_to_cynthia`) SHALL be designated hold-out (not used for any calibration decision).

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file inside the change folder at every verification point, so the acceptance evidence is inspectable without re-running. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented in this change with explicit user sign-off); or FAIL (blocks the change). A synthetic subset of the gate (the frame/split/attachment rules on recorded fixture arrays) SHALL run in plain `cargo test` without audio or models.

#### Scenario: Fixture gate validates the engine before review

- **GIVEN** the meeting audio, local models, and the gate env set
- **WHEN** the gate test runs
- **THEN** every ear-truth entry passes against the derived turns, or each non-passing entry carries a recorded KNOWN-LIMITATION with user sign-off
- **AND** the recorded output file in the change folder reflects the latest run

#### Scenario: Fixture failure blocks the change

- **GIVEN** an engine change that moves a pinned boundary or flips a pinned label
- **WHEN** the gate test runs
- **THEN** it fails, naming the violated entry
- **AND** the entry may only be resolved by passing the engine or by user-confirmed KNOWN-LIMITATION

#### Scenario: Synthetic gate subset runs in CI

- **GIVEN** a plain `cargo test` without meeting audio or models
- **WHEN** the gate's synthetic subset runs
- **THEN** the run/split/attachment rules are asserted against recorded fixture arrays without env gates

### Requirement: Persisted turns carry an engine-emitted continuation fact

The engine SHALL persist, per derived turn, whether it continues the previous turn's sentence (`continues_previous`): true when the turn resulted from a backward-attached ambiguous piece, same-label adjacency across an absorbed gap, or the turn's first text begins mid-sentence (`effective_continuation` — the machine marks lowercase-initial turns, never presents them as fresh starts); false at corroborated voice changes whose text begins a fresh sentence. The flag SHALL be stored on the turn's FIRST persisted row (a nullable `transcripts` column added by migration); the UI resolves a turn's flag from its first row. The transcript UI SHALL render the engine fact as the continuation marker when present and fall back to the existing text heuristic (`isContinuation`) only when the flag is null; when both exist and disagree, the engine fact wins. A false continuation marker SHALL be treated as an attribution defect: the fixture gate SHALL assert marker correctness on its pinned `voice_change_at` entries.

**No unmarked mid-sentence cuts (hard invariant):** every persisted turn whose text begins mid-sentence (lowercase-initial after stripping leading punctuation/symbols/whitespace) SHALL carry `continues_previous = true`. A mid-sentence-initial turn without the flag is an attribution defect, and the gate test SHALL enforce this over the ENTIRE meeting output — every turn, not only pinned fixture spans — failing with the offending turn's time and text on violation. A turn may only begin mid-sentence if it is a declared continuation; silence, gaps, and voice changes do not excuse an unmarked one.

#### Scenario: Engine fact drives the marker

- **GIVEN** a turn produced by backward attachment of an ambiguous piece
- **WHEN** the transcript renders
- **THEN** the turn is marked as continuing the previous sentence from the persisted flag, regardless of the text heuristic

#### Scenario: No unmarked mid-sentence-initial turn survives the gate

- **GIVEN** the full persisted output of a diarization run
- **WHEN** the gate scan runs over every turn
- **THEN** every turn starting mid-sentence carries `continues_previous = true`
- **AND** any violation fails the gate naming the turn's start time and leading text

#### Scenario: Marker correctness is gated

- **GIVEN** the fixture's `voice_change_at` entries
- **WHEN** the gate test runs
- **THEN** the turn following the pinned boundary carries `continues_previous = false` (it must not render as a continuation of the earlier turn)

### Requirement: Re-diarization re-derives manually-labeled rows and re-applies names

When the user explicitly re-runs diarization on a meeting that contains manually labeled (renamed) transcript rows, the system SHALL re-derive ALL rows including the manual ones, and SHALL re-apply user-applied speaker names afterward via the stamped-embedding match (the persisted named-speaker pool); names that cannot be re-matched SHALL be reported to the user in the run result rather than silently dropped. Meeting-local names whose only voice evidence was deleted by the run's own stale-state cleanup cannot re-match by construction — they are reported as unmatched, which is the accepted limitation. The pre-run manual labels SHALL be enumerated before stale-state cleanup so the report is complete. The previous behavior of leaving manual rows frozen on old boundaries is retired; the manual-row guard is relaxed only on the explicit re-run path (automatic write paths keep the guard). Automatic (unnamed) rows are unaffected by this requirement.

#### Scenario: Renamed speaker survives a re-run

- **GIVEN** a meeting where the user renamed a cluster to "Cynthia" and a stamped embedding exists for it
- **WHEN** diarization re-runs with the new engine (different cluster count/ordering)
- **THEN** all rows re-derive with new boundaries
- **AND** the rows whose voice matches the stamped "Cynthia" embedding are labeled "Cynthia"
- **AND** any user name that could not be re-matched is reported in the run result

### Requirement: Persisted speaker turns are sentence-readable

The diarization persist path SHALL write speaker turns, not raw aligned fragments: adjacent rows of the SAME speaker with a time gap of at most 3 seconds SHALL merge into one turn (text joined in time order); rows that carry no alphanumeric content SHALL be dropped; word-joined text SHALL be detokenized (no space before sentence punctuation, contractions reattached). Rows of DIFFERENT speakers SHALL never merge, even across a mid-sentence interjection.

#### Scenario: Same-speaker fragments merge into a readable turn

- **WHEN** alignment produces "Speaker 0: `. Okay . I have some updates . Cool . On the`" followed 0.4 s later by "Speaker 0: `to , let 's , wait`"
- **THEN** one persisted row for Speaker 0 reads "Okay. I have some updates. Cool. On the to, let's, wait" spanning both time ranges
- **AND** the original fragments no longer exist as separate rows

#### Scenario: Backchannel fragment merges; junk rows disappear

- **WHEN** a 0.5 s row reading "Yeah ," sits between two Speaker 1 rows, and a row reading "," sits anywhere
- **THEN** the "Yeah ," row merges into its neighboring same-speaker turn
- **AND** the punctuation-only row is dropped entirely

#### Scenario: Speaker flip never merges, mid-sentence

- **WHEN** Speaker 0's fragment "On the" is followed by Speaker 1's "roadmap , hopefully"
- **THEN** both persist as separate rows (the interjection is real)
- **AND** Speaker 0's text is detokenized ("On the", no trailing-space artifacts)

#### Scenario: Silence longer than 3 seconds starts a new turn

- **WHEN** two same-speaker rows are separated by more than 3 seconds of gap
- **THEN** they persist as separate turns

### Requirement: Consolidation of already-persisted meetings is transactional and idempotent

A consolidation pass SHALL apply the same assembly to a meeting's persisted rows within a single transaction (insert merged turns, delete absorbed rows). Running it twice SHALL produce the same final rows. Consolidation SHALL NOT require audio or re-diarization.

#### Scenario: Consolidating an already-consolidated meeting is a no-op

- **WHEN** consolidation runs on a meeting whose rows are already assembled turns
- **THEN** row texts, counts, speakers, and ids are unchanged

#### Scenario: Consolidation failure leaves the meeting untouched

- **WHEN** the transaction fails mid-pass
- **THEN** the meeting's rows are exactly as before the attempt

### Requirement: Sentences are not split across speaker badges

The diarization assembly SHALL treat the transcript sentence as the atom of speaker assignment. Persisted fragment rows that are adjacent in the persisted sequence and whose predecessor lacks sentence-terminal punctuation SHALL first be REJOINED into one logical text unit (a text-level repair spanning badges and row gaps — not a badge merge; rejoined text preserves word order and content). Each sentence (segmented from the logical units, with a time span from valid token timestamps or a proportional share of the unit's span) SHALL then be assigned WHOLE to one speaker badge: the engine turn owning the majority of the sentence's span. A sentence SHALL NOT be divided across two speaker rows. The engine's turn boundaries themselves are unchanged, and this SHALL NOT be implemented as a merge of two speakers' rows — minority-span words move with their sentence (reattribution), and a turn whose span holds no assigned sentence emits no row.

**No bounded-run guard (user ear decree, 2026-09-09 — supersedes the original 8 s cut design)**: the voice does NOT change mid-sentence in the user's meetings. Every engine boundary inside a sentence is an engine error, absorbed by whole-atom majority assignment; sentence atoms are never cut and never flagged as continuation tails. The bounded-run cut, its `cross_badge_tail` flag, and the fracture waiver class are RETIRED. The fix for a wrong badge is engine boundary accuracy (change `engine-boundary-and-identity-accuracy`), never cutting the sentence.

The render gate SHALL count cross-badge fractures with the predicate "row begins mid-sentence AND the previous persisted row has a different badge AND that previous row does not end with terminal punctuation" and SHALL fail on any fracture with NO waiver path — there is no amendment-record waiver for fractures. Lowercase sentence onsets that are the ASR text itself (no cross-badge fracture) are NOT violations. The ear gate SHALL additionally count duplicate clusters and overlapping-span rows (same-audio double-decode suspects), and report churn counters (persisted rows per minute; rows of ≤2 words).

Adjacent persisted rows whose normalized token sequences share a contiguous subsequence of at least 3 tokens covering at least 80% of the shorter row — with disjoint time spans, an inter-row gap of at most 2 seconds, and different badges (or one side "Unknown Speaker") — form a duplicate cluster (a chunk-overlap re-transcription). Assembly SHALL resolve a duplicate cluster by keeping ONE copy — survivor badge: labeled over "Unknown Speaker"; among multiple labeled rows, the badge owning the majority of the cluster's union span; final tiebreak earliest start — writing its text ONCE, extending the survivor's span to the union of the cluster's spans, and deleting the absorbed row shells inside the persist transaction; no source row may survive the transaction as an unlabeled orphan. The survivor's time coverage SHALL never shrink (audio-time evidence is preserved).

#### Scenario: Turn flip inside a sentence keeps the sentence whole

- **WHEN** the sentence "Where is Ricardo?" spans a speaker flip between Speaker 1's turn and Speaker 0's turn
- **THEN** the full sentence persists under exactly one badge (the majority-span badge)
- **AND** no row under the other badge contains any part of that sentence
- **AND** the engine's turn set still contains the flip (engine boundaries are not edited)

#### Scenario: Mid-sentence tail under a different badge is repaired by rejoin

- **GIVEN** persisted fragment rows "I" (Speaker 1, 39.27–39.93) followed by "don't know. Let me ping in..." (Speaker 0), where "I" lacks sentence-terminal punctuation
- **WHEN** assembly rejoins the adjacent fragments into one logical unit, segments it into sentences, and assigns each sentence to its majority-span badge
- **THEN** "I don't know." persists whole under Speaker 0 (the majority badge), not split as "I" under Speaker 1 with the tail under Speaker 0
- **AND** any residual fracture that cannot be repaired trips the gate with the offending rows

#### Scenario: ASR-lowercase onset without fracture is not a violation

- **GIVEN** a row that begins with a lowercase word because the ASR text itself is unpunctuated lowercase (e.g. "yeah i think it's two sprints..."), whose previous row ends with terminal punctuation or carries the same badge
- **WHEN** the render gate's fracture predicate evaluates it
- **THEN** the row is not counted as a violation

#### Scenario: Short interjection does not slice the host sentence

- **WHEN** a short interjection sentence ("Yeah,") from Speaker 0 lands between two sentences of Speaker 1
- **THEN** the interjection persists as its own row under Speaker 0
- **AND** both surrounding Speaker 1 sentences persist whole under Speaker 1

#### Scenario: Duplicate re-transcription cluster is merged, not double-persisted

- **GIVEN** two rows 1.1 s apart with disjoint spans, different badges, whose texts share the contiguous 6-token sequence "motors we're doing the feature flag update" covering ≥80% of the shorter row
- **WHEN** assembly resolves the duplicate cluster
- **THEN** one row survives carrying the shared text once, with its span extended to the union of the cluster's spans
- **AND** the absorbed row's shell is deleted in the same transaction and no unlabeled source row remains
- **AND** a genuine short repeat ("I can't. I can't." inside one row, or a full-sentence interjection) is not a duplicate cluster (its atoms are not a ≥3-token contiguous cross-badge match with disjoint spans)

#### Scenario: Render-level residuals other than fractures are waivable only by amendment record

- **WHEN** a render-level gate assertion (duplicate, unknown-within-cap, zero-duration, unmerged) has an irreducible residual on the pinned fixture
- **THEN** the gate fails unless the fixture carries a user-signed amendment record for that offender (date + reason), the same mechanism as fixture-entry amendments
- **AND** a cross-badge fracture has no such path — since the 2026-09-09 ear decree every fracture fails hard

### Requirement: Diarization aligns from an immutable transcription source

The system SHALL maintain an immutable per-meeting copy of the transcription rows (the
rows exactly as the transcription engine wrote them: full text, sentence punctuation,
token timestamps, original row boundaries), held in `transcript_sources` with a
`source_origin` provenance value ('stt' for transcription-lane writes, 'backfilled'
for migration-seeded rows). The speaker pipeline SHALL read its alignment input and
engine transcript priors from this copy on EVERY run, never from the rendering rows it
wrote on a previous run. The speaker pipeline SHALL NOT modify or delete source rows;
its persist step SHALL replace only rendering rows (via full-meeting regeneration).
Meeting deletion SHALL remove the meeting's source rows explicitly within the deletion
transaction.

#### Scenario: Consecutive runs from a pinned source are idempotent at the alignment+persist stages

- **GIVEN** a meeting whose immutable source holds N transcription rows and FIXED
  synthetic diarization segments (engine stage excluded — it is recomputed per run and
  not asserted deterministic by this requirement)
- **WHEN** the align + persist pipeline runs twice from the same source
- **THEN** both runs produce the same rendering (same row count, same texts, same
  spans, same badges; freshly-generated row ids excepted)
- **AND** no run's output feeds any later run's input

#### Scenario: Absorbed and duplicate-resolved source text survives in the source copy

- **GIVEN** a source row whose text alignment absorbs into a neighboring sentence atom,
  or a resolved duplicate-cluster member
- **WHEN** regeneration replaces the rendering
- **THEN** the rendering may omit the absorbed text (assembly resolved it)
- **AND** the source copy still contains the absorbed row's original text, punctuation,
  and span, so any later run re-derives from the same evidence

#### Scenario: Legacy-degraded meeting freezes instead of degrading

- **GIVEN** a meeting whose backfilled source (`source_origin = 'backfilled'`) holds
  degraded, previously-diarized rows
- **WHEN** the Speakers pipeline runs any number of times
- **THEN** the rendering is a pure re-derivation of that frozen source
- **AND** the source itself is unchanged, so no further text structure is lost

### Requirement: Transcription writes seed the immutable source

Every fresh-transcription writer SHALL write the source copy and the rendering rows
together in one transaction, through a single shared repository helper — meeting
import, retranscription, and any future transcription-lane writer. Retranscription
SHALL delete and rewrite BOTH tables for the meeting, regenerating the immutable
source from the new engine output.

#### Scenario: Retranscription heals a legacy-degraded meeting

- **GIVEN** a meeting whose backfilled source is frozen, degraded diarization output
- **WHEN** the user re-transcribes the meeting and then runs the Speakers pipeline
- **THEN** the source copy holds the new transcription rows with full sentence
  punctuation and token timestamps
- **AND** the aligned rendering is derived from the healed source, not from any
  pre-retranscription row

#### Scenario: Dual-write drift is observable, not silent

- **GIVEN** a meeting whose rendering rows are non-empty while its source rows are
  absent (a writer that bypassed the helper, or rows written by a pre-split binary)
- **WHEN** the Speakers pipeline runs
- **THEN** the run emits a warning log identifying the meeting and the divergence
  (the alignment input is empty; the run does not invent input from rendering rows)

### Requirement: Sentence punctuation survives the rendering pipeline

The rendering pipeline's text behavior SHALL satisfy: every SOURCE sentence containing
at least one alphanumeric character SHALL have its terminator present in at least one
persisted rendering row. Duplicate-cluster absorbed members, punctuation-only
ranges, and audio claimed by a surviving manually-corrected rendering row are
explicitly exempt (a resolved duplicate writes its text once; a
punctuation-only atom is dropped by design; a manual row renders its own text). The joins that build rendering text
(same-label fragment merge, consolidation) SHALL preserve the terminators of the texts
they join. A fixture-driven test SHALL assert this invariant on a fixture containing
multi-sentence source rows.

#### Scenario: Question mark survives repeated alignment

- **GIVEN** a source row "Where is Ricardo? I don't know. Let me ping in. I can't."
- **WHEN** the Speakers pipeline aligns and persists it, any number of times
- **THEN** each of the four sentences has its terminator present in the persisted
  rendering text (across one or more rows)
- **AND** the whole-atom aligner can re-derive the same sentence atoms on the next run

