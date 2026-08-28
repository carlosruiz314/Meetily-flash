## ADDED Requirements

### Requirement: Embedding rows persist with a resolved speaker identity

The live diarization path SHALL persist every speaker centroid embedding with a non-null `speaker_id`. Clusters whose centroid matches a named speaker SHALL link to that speaker's id; unmatched clusters SHALL link to a meeting-local auto-created speaker row (`speaker-auto-{meeting_id}-*`). The replacement of a meeting's embedding set (delete stale rows, insert stamped rows) SHALL run in a single transaction, and a store failure SHALL fail the run rather than be swallowed.

The matcher SHALL ignore embeddings with a null `speaker_id`. Null is a legitimate steady-state value for deliberately unlinked embeddings (revert, speaker deletion); no code path other than those user actions SHALL produce new null rows.

#### Scenario: Unmatched cluster links to a meeting-local auto row

- **WHEN** diarization completes and a centroid does not match any named speaker above the configured threshold
- **THEN** the embedding is stored with the meeting's own auto speaker row id as `speaker_id`

#### Scenario: Matched cluster links to the named speaker

- **WHEN** diarization completes and a centroid matches a named speaker above the configured threshold
- **THEN** the embedding is stored with the named speaker's id as `speaker_id` and no duplicate `speakers` row is created

#### Scenario: Failed store rolls back to the previous stamped set

- **WHEN** an insert of a stamped centroid fails mid-run
- **THEN** the transaction rolls back, the meeting's previous embedding set remains intact, and the run reports an error

#### Scenario: Deleting a meeting prunes its auto speaker rows

- **WHEN** a meeting is deleted
- **THEN** its embeddings are removed (existing cascade) and the meeting's `speaker-auto-{meeting_id}-*` speaker rows are deleted as well

## MODIFIED Requirements

### Requirement: Cross-meeting speaker matching uses embedding similarity

After diarization assigns anonymous speaker labels ("Speaker 0", etc.), the system SHALL compare each speaker cluster's centroid embedding against the embeddings of named speakers — rows in `speaker_embeddings` whose `speaker_id` references a named speaker — using cosine similarity. The pool SHALL be keyed by `speaker_id`, never by speaker name or cluster label, and auto-created meeting-local speaker rows SHALL NOT be match candidates. The pool SHALL be loaded from the database at the start of every Speakers run so that renames and prior runs within the same app session are visible. Matches above the threshold SHALL auto-label the cluster with the matched speaker's name and link the stored embedding to that speaker's id.

The threshold SHALL default to 0.40 and SHALL be configurable via advanced settings in range [0.35, 0.70]; the matcher SHALL use the configured threshold, not a hard-coded constant.

#### Scenario: Matching speaker auto-labeled

- **GIVEN** "Alice" is a named speaker with stamped embeddings in the pool
- **WHEN** diarization produces a cluster whose centroid cosine similarity to Alice's pool vectors is at or above the configured threshold
- **THEN** the cluster is labeled "Alice" and its stored embedding links to Alice's speaker id

#### Scenario: No match produces cluster label

- **GIVEN** no named-speaker pool vector has similarity at or above the configured threshold
- **WHEN** diarization produces a speaker cluster
- **THEN** the cluster keeps its cluster label ("Speaker 0") and links to the meeting-local auto speaker row

#### Scenario: Another meeting's unnamed cluster is not a match candidate

- **GIVEN** meeting B previously produced an auto row named "Speaker 0" with a stamped embedding
- **WHEN** meeting A runs diarization
- **THEN** meeting B's auto row contributes no vectors to A's match pool

#### Scenario: Rename within a session is visible to the next run

- **GIVEN** the user renamed a cluster to "Cynthia" after the app started
- **WHEN** Speakers runs on another meeting in the same session
- **THEN** the match pool reflects the rename (vectors resolved by speaker id, current name displayed)
