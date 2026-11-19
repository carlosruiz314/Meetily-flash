## MODIFIED Requirements

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
`speaker_source = 'auto'`, `duration` recomputed from its own timing, and every other
column copied from its SOURCE row (joined via the segment's source-row id) EXCEPT
`token_timestamps`, which the row carries from the alignment: when the segment's text
derives from token-walled source words, the row persists the per-word walls of exactly
the words its text holds (the `TokenWord` JSON shape the source carries), and when the
segment derives from token-less source words it persists `token_timestamps = NULL`;
(5) if the aligned output is empty while the source table is non-empty, abort the
transaction (a degenerate run never wipes the rendering). A scheme that mutates prior
rendering rows by id lookup (last-writer-wins UPDATE, or a delete-by-input-id sweep
that treats previous output as absorbed) SHALL be considered NON-CONFORMANT: input row
ids identify SOURCE rows, and rendering row ids have no stable relationship to them
across runs.

Rendering rows from regeneration carry their aligned token walls; `token_timestamps =
NULL` persists ONLY where the segment's words derived from token-less source rows
(the legacy fallback). Rows spliced by the repair path (synth atoms) are owned by the
repair capability and persist as it specifies (NULL walls today) — this requirement
governs regeneration output only. A persisted row's `previous_label` SHALL be NULL
(previous_label is rendering history carried by manually-labeled rows, which survive
regeneration untouched; fresh regenerated rows have no label history).

Both merge layers that build persisted rows SHALL preserve walls verbatim: the
engine-side same-label fragment merge (`merge_same_label_fragments`, applied between
alignment and persist) and the post-persist consolidation merge
(`consolidate_meeting_turns`) concatenate the merged rows' token walls in row order —
never re-normalized, re-stamped, or re-derived; any wall-less member (or unparseable
wall JSON) fails the whole list closed to NULL (a merged row mixing walled and
wall-less pieces cannot carry a wall-faithful list). Wall integrity SHALL hold for
every persisted row carrying walls: each token wall sits within the row's own
`audio_start_time`/`audio_end_time` bounds, the start times are monotone
non-decreasing in list order, and no wall ends before it starts. The wall list SHALL
be wall-faithful to the row's text: the alphanumeric words of the row's text equal
the alphanumeric words of its persisted token list in order, compared AFTER the same
detokenize reflow the render's own turn text underwent (the ear gate's token
semantics — a reflow that joins two wall words into one text word, e.g. "don" + "'t"
rendering as "don't", is wall-faithful, not a violation). The integrity check SHALL
be one shared predicate for every check site (the post-persist census and the
persist gate), so the sites cannot drift. The no-diarization passthrough path has no
validator gate in front of it, so it SHALL apply the integrity check directly: a
passthrough row persists its source walls only when the row's own text spells them
(and they satisfy bounds, monotonicity, and end >= start); otherwise it persists
NULL.

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
shape is derived, never accumulated.

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
pipeline, where words legitimately move between rows via reattribution. Where the
source rows carry token walls, the per-row form is strengthened: each rendering row's
walls cover exactly its own text's alphanumeric words, in order.

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
  id and recomputed duration
- **AND** each persisted row carries its own words' token walls under
  `token_timestamps` (the Speaker 0 row carries the walls of tokens 5.0–5.4, the
  Speaker 1 row the walls of tokens 7.3–7.7)
- **AND** every other column equals the SOURCE row's values (copied from
  `transcript_sources`, not from any prior rendering row)

#### Scenario: Consolidation merge concatenates token walls verbatim

- **GIVEN** two rendering segments under the same badge whose gaps sit inside the
  consolidation merge cap, each carrying its own token walls
- **WHEN** consolidation merges them into one row
- **THEN** the merged row's `token_timestamps` is the concatenation of the two lists
  in row order, with each wall preserved verbatim
- **AND** the merged row's walls satisfy wall integrity (inside the merged row's own
  bounds, monotone non-decreasing, matching the merged text's alphanumeric words)

#### Scenario: Run log records a token-only wall census after consolidation

- **GIVEN** a Speakers run that persisted and consolidated a meeting's rendering
- **WHEN** the run completes
- **THEN** the run log carries a census line with the meeting's row count, the
  walled-row count, the NULL-wall count, and the wall-integrity violation count
  (token-only: counts and row bounds, never content)

#### Scenario: Detokenize reflow is wall-faithful, not a violation

- **GIVEN** a persisted row whose text reads "don't" (one detokenized word) and
  whose wall list holds "don" and "'t" as two entries
- **WHEN** wall integrity is evaluated (census or invariant)
- **THEN** the row is wall-faithful — both sides are compared after the same
  detokenize reflow, and the row counts as walled with no violation

#### Scenario: Wall integrity holds on every walled row

- **GIVEN** a meeting whose source rows carry token timestamps
- **WHEN** the Speakers pipeline persists any regeneration
- **THEN** every persisted row with non-NULL `token_timestamps` satisfies: each token
  start/end within the row's own audio bounds, start times monotone non-decreasing,
  no wall ending before it starts, and the list's alphanumeric words equal the row
  text's alphanumeric words in order
- **AND** a row whose segment derived from token-less source words persists NULL and
  renders by the unchanged proportional fallback

#### Scenario: Passthrough rows persist only walls their own text spells

- **GIVEN** a run whose diarization output is empty (the passthrough path), where one
  source row's wall list names words its text does not spell (a measured real-corpus
  shape: source chunk [1010.07,1032.31] carries 68 wall pieces against 50 text words)
  and another source row's walls match its text
- **WHEN** alignment runs the passthrough and persists
- **THEN** the mismatched row persists `token_timestamps = NULL` (fail closed, no
  partial claim) while the matching row carries its walls verbatim (eot markers
  dropped)

#### Scenario: Fragment merge concatenates token walls verbatim

- **GIVEN** two adjacent fragments under the same speaker label, each carrying its
  own token walls
- **WHEN** the engine-side fragment merge joins them into one segment
- **THEN** the merged segment's wall list is the concatenation of the two lists in
  fragment order, each wall preserved verbatim
- **AND** a merge mixing a walled fragment with a wall-less fragment fails closed to
  a wall-less (NULL) segment

#### Scenario: Second consecutive run does not shrink or corrupt the rendering

- **GIVEN** a meeting whose source holds N rows and whose rendering holds the output of
  a previous Speakers run (fresh-UUID rows that share no ids with the source rows)
- **WHEN** the Speakers pipeline runs again with the same diarization segments
- **THEN** the rendering is rebuilt to the same row count and the same (text, span,
  badge, token-wall) multiset as before the run
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
