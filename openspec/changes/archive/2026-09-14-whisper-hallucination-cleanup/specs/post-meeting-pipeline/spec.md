## ADDED Requirements

### Requirement: Batch transcription quarantines hallucinated segments

Batch transcription SHALL resolve the transcription language ONCE per run from the
user's language preference as held in `LANGUAGE_PREFERENCE` (synced from the UI;
`auto`, `auto-translate`, and unset all resolve to `auto-translate` — detect and
translate to English; an explicit ISO code resolves to that code) and SHALL pass that
resolution to every decode in the run. Every decoded segment SHALL pass a text-level
hallucination audit before persistence. A flagged segment SHALL be retried once with a
deliberately different decode (greedy sampling, temperature 0, language pinned to the
run's concrete language — `en` when the run resolved `auto-translate`), and only if the
retry is also flagged SHALL the segment be dropped from the persisted transcript set;
every drop SHALL be logged and counted, and a run whose drops exceed 30 % of its
segments SHALL fail instead of persisting a gutted transcript. Resume paths SHALL
audit checkpointed text under the same rule and delete still-flagged checkpoint rows.

#### Scenario: The queue no longer drops the language preference

- **WHEN** a batch run transcribes a meeting with the preference at `auto`,
  `auto-translate`, or unset
- **THEN** the run decodes with `auto-translate` (per-window detect + translate to
  English), not `None`
- **AND** when an explicit ISO code is set, every segment is decoded with that code and
  no segment is independently language-detected

#### Scenario: Flagged segment is retried with a different decode before it is dropped

- **WHEN** a decoded segment is flagged by the audit (U+FFFD, non-Latin script chars,
  dominant token loop, or absurd token rate)
- **THEN** the same segment samples are re-decoded once with greedy sampling,
  temperature 0, and the run's concrete language pinned
- **AND** a clean retry text replaces the flagged text (and its checkpoint)
- **AND** a still-flagged retry is dropped from the persisted set, logged with its
  timestamps and trigger stats, and counted

#### Scenario: Mass drop aborts the run instead of publishing a gutted transcript

- **WHEN** the count of dropped segments exceeds 30 % of the run's segments
- **THEN** the job fails and no transcript rows are written for the meeting

#### Scenario: Clean rows are never damaged

- **WHEN** the audit runs over the pinned cde5c264 fixtures (240-row pre-live and
  173-row current snapshots)
- **THEN** exactly the rows listed in the tracked expectations file (keyed by each
  fixture's `row_sha256`) are flagged
- **AND** no other row is flagged

#### Scenario: Existing meetings can be repaired offline

- **WHEN** the repair harness runs against a stored meeting's rows and its audio file
- **THEN** each flagged row is re-decoded with the different-decode retry from
  sample-sliced windows of the flagged spans
- **AND** a repaired-clean row replaces the original text (token_timestamps and
  speaker label reset); a still-flagged row is dropped and its full text reported
- **AND** the report-only mode writes nothing; the write mode backs up the original
  rows to JSON before rewriting only the replaced rows in one transaction
- **AND** after the repair, no row of the meeting is flagged by the same audit
