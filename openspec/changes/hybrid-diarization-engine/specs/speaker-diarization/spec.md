## ADDED Requirements

### Requirement: Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits

On the success path (pyannote segmentation model present), the system SHALL derive speaker turns in two layers:

1. **Speech runs**: slide the production 10s/1s pyannote window grid over the full meeting (the final <1s tail SHALL be decoded with zero-padding so trailing speech is not silently dropped) and decode per-frame speaker-probability masses from the powerset output; collapse the speech-vs-silence track (speech = summed speaker mass > 0.5) into runs with minimum duration 0.3s. Min-duration collapsing SHALL absorb silence runs and same-label fragments only; a short run with a DIFFERENT label than its neighbor SHALL be retained as a piece and resolved by the attachment rules below.
2. **Sub-run splits**: a run containing a label-track change is split at the change point only where the change is corroborated: the two windows adjacent to the split time, each decoded independently, MUST each show a label-change event at the same time (agreement on the change EVENT and its timestamp within a ±0.35s tolerance — calibrated: per-window decodes jitter more than 0.2s at real changes — window-local index identities are permutation-ambiguous and are never compared across windows). Windows whose decodes disagree at the split point are treated as seam permutations and rejected; the run stays whole. A mode filter over the label track (not the per-speaker bool median filter, which does not apply to argmax labels; radius 3 frames) removes single-frame label flicker before split detection.
3. **Labeling**: each piece of duration ≥1.5s (the `MIN_SPEECH_SECS` floor; the model's minimum embedding input) SHALL be embedded — pieces longer than 12s embed their middle 12s, matching the validated measurement — and clustered by threshold clustering at the configured merge threshold with deterministic tie-breaking. Cluster count SHALL be capped by the meeting's max_speakers cap using the production most-isolated-cluster merge policy (`enforce_max_speakers_cap` semantics), and a nearest-centroid refinement pass SHALL reassign every labeled piece to its final centroids. Every persisted centroid SHALL correspond to at least one persisted labeled piece (no phantom speakers).
4. **Attachment**: a piece below the 1.5s floor MAY be embedded solely for attachment and SHALL attach to the temporally PREVIOUS labeled piece's turn (the following turn only when no previous exists); a piece whose final-centroid similarity margin between best and second-best is below a positive ambiguity margin (default 0.05, computed after the refinement pass) SHALL attach backward the same way. Attachment SHALL never create a new label, and contiguous backward-attached material exceeding 5s within one turn SHALL instead form its own turn flagged low-confidence in the data. Sub-floor arbitration is fixture-calibrated (design D3 amendment): a sub-floor piece at/above a promotion floor (0.8s; shorter slices win on noise) with a decided best cluster (margin ≥ the ambiguity margin) forms its own low-confidence turn; a sub-floor piece whose neighbors AGREE on one cluster joins them (burst fragmentation of one speaker's speech); a sub-floor piece sandwiched between two DIFFERENT clusters keeps the boundary as its own low-confidence turn with a best-effort label — a real interjection the 1.5s floor must not silently absorb.
5. **Textless runs**: runs with no transcript-text overlap (breaths, laughs, untranscribed voiced noise; detected with a whisper-timestamp skew tolerance) SHALL be dropped BEFORE same-cluster coalescing, so a textless run can never split one speaker's stretch into fragments.
6. **Coalescing and text alignment**: same-cluster neighbors separated only by dropped runs or absorbed silence SHALL coalesce, and the engine's turns ARE the persisted turn units on the success path — post-persistence turn merging layers (legacy consolidation, persist-path assembly) SHALL NOT re-run over engine output; where such a merge ever applies, `continues_previous` is re-derived from the merged members (first member's flag). Every transcript text row SHALL land in exactly one turn. Rows with token timestamps SHALL split at turn boundaries (token-level alignment); rows without token timestamps (legacy consolidated rows) SHALL split proportionally at any turn boundary falling inside them; a row with zero time overlap with every turn (possible only in a shed span) SHALL attach to the nearest-in-time turn. This content-preservation invariant SHALL be asserted end-to-end (every input row's alphanumeric content appears in the persisted output).
7. **Overlap flags**: a turn's crosstalk flag SHALL be the fraction of its FINAL merged span whose per-frame overlap-pair probability mass (sum of powerset classes 4–6) exceeds 0.25, recomputed after all merging; fragment-maximum aggregation is forbidden. Overlap flags are persisted but not rendered by this change.
8. **Determinism**: all label decisions SHALL be computed from time-ordered or index-ordered sequences (ordered containers; no HashMap-iteration-order effect on labels, boundaries, or flags), so identical audio, models, and settings on the same binary produce identical turns. The clustering step itself SHALL be covered by a CI-runnable determinism unit test on synthetic embeddings.

Where this requirement and the chunk-grid labeling requirements conflict on the success path, this requirement governs; the pyannote-model-missing fallback path is unchanged. The number of embedded pieces per meeting SHALL be bounded by a shed-to-cap applied to runs before embedding (cap 2000, shed by position, merging sub-floor survivors within same-label spans of their speech region — never across a corroborated voice change), so clustering cost is bounded regardless of meeting length. Shedding is permanent on this path (no pass-2 re-labeling exists); spans lost to the cap lose attribution. If the embedding model fails to load, the run SHALL fail with the error surfaced (no partial labels); if pyannote inference fails mid-pass, the run SHALL fail without persisting partial labels. The label-track mode filter SHALL use radius 3 frames (≈50ms), calibratable only under the fixture-gate rule.

#### Scenario: One voice across a mid-sentence pause stays one turn

- **GIVEN** the cde5c264 fixture entry that 9.38–13.03s is one voice
- **WHEN** the engine derives turns
- **THEN** no turn boundary exists between 9.38s and 13.03s (in particular not at the whisper-row edge 12.07s)
- **AND** the turn covering the span carries the same label as the speaker's other clean runs

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

#### Scenario: Every transcript row survives into the output

- **GIVEN** any diarization run on a meeting with transcript rows
- **WHEN** turns are assembled and persisted
- **THEN** every input row's alphanumeric content appears in exactly one output turn
- **AND** a token-timestamped row straddling a turn boundary splits at that boundary; a token-less row splits proportionally; a zero-overlap row attaches to the nearest-in-time turn

#### Scenario: Corroborated text tail lands on the earlier turn

- **GIVEN** the 02:12–02:50 fixture entry, where the "And I was like, oh, when you put a that one" tail shares a whisper row with later-speaker text
- **WHEN** the rows are aligned to the derived turns
- **THEN** the row splits at the ≈161s boundary and the tail text is attributed to the earlier turn (whole-row assignment would make this entry structurally unpassable)

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

## MODIFIED Requirements

### Requirement: Transcript-timestamp-driven speaker diarization runs as a post-processing queue phase

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
- **Amendments to untouched requirements (success-path scope)**: "Temporal-coherence smoothing prevents clustering contamination and per-chunk flicker" (canonical ~line 690), "Diarization segment granularity resolves speaker turns within Whisper segments" (~line 784), "Short chunks are not attributed to temporally-absent speakers" (~line 867), and "Short-duration noise speakers are merged into nearest cluster" (~line 93, whose `MIN_CLUSTER_FRAC` merge is superseded by the attachment rules) henceforth govern ONLY the fallback path; "Centroid embeddings are stored per speaker per meeting" (~line 250) is amended so that on the success path stored centroids derive from run/piece embeddings and contain no smoothing-refinement clause; "Token-level timestamps align transcript text with diarization speaker boundaries" (~line 108) governs token-timestamped rows and is amended so that token-less rows split proportionally at turn boundaries and the manual-row guard is scoped per the re-diarization requirement above; "Re-transcription clears and re-enqueues diarization" (~line 491) is re-pointed to the `run_diarization_for_meeting` transport. At archive these requirement blocks are updated with this scoping, and the headline requirement is RENAMED to drop the stale "queue phase" phrasing.
