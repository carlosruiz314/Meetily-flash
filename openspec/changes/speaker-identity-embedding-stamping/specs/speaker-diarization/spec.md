## ADDED Requirements

### Requirement: Speaker centroid embeddings persist with a concrete speaker identity

The diarization pipeline SHALL persist every speaker centroid embedding with a non-null `speaker_id` referencing a `speakers` row. Clusters matched against prior meetings SHALL link to the matched speaker's id; unmatched clusters SHALL create a new `speakers` row and link to it. The pipeline MUST NOT persist embeddings with `speaker_id = NULL`.

#### Scenario: Unmatched cluster creates and links a new speaker

- **WHEN** diarization completes and a centroid does not match any existing speaker above the similarity threshold
- **THEN** a new `speakers` row is created and the embedding is stored with that row's id as `speaker_id`

#### Scenario: Matched cluster links to the existing speaker

- **WHEN** diarization completes and a centroid matches a speaker from a prior meeting above the similarity threshold
- **THEN** the embedding is stored with the matched speaker's id as `speaker_id`, and no duplicate `speakers` row is created for it

### Requirement: Cross-meeting matching consumes only identity-stamped embeddings

The cross-meeting matcher SHALL source its embedding pool exclusively from rows where `speaker_id IS NOT NULL`. Rows with a null `speaker_id` MUST be ignored by matching and MUST NOT be pooled by cluster label.

#### Scenario: Legacy NULL rows do not contaminate matching

- **WHEN** the matcher runs while unstamped legacy rows exist in `speaker_embeddings`
- **THEN** those rows contribute no vectors to the match pool and matching proceeds from stamped rows only

#### Scenario: Rename propagates identity across meetings

- **WHEN** a user renames a cluster to an existing speaker's name and later runs Speakers on another meeting
- **THEN** the new meeting's matching centroid links to that speaker's id

### Requirement: Re-running Speakers replaces a meeting's embeddings atomically

A Speakers run on a meeting SHALL delete that meeting's previous embedding rows before storing the new stamped set, so the table never holds duplicate centroids for the same meeting.

#### Scenario: Re-diarization leaves one stamped set per meeting

- **WHEN** Speakers is run a second time on a meeting that already has stamped embeddings
- **THEN** the old rows for that meeting are gone and every new row for the meeting carries a non-null `speaker_id`
