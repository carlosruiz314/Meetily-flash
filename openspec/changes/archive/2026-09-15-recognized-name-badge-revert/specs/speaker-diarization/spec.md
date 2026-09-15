## MODIFIED Requirements

### Requirement: Retroactive speaker labeling via inline badges with per-speaker revert

The frontend SHALL render an inline speaker badge next to each transcript segment. The badge SHALL display the current speaker label ("Speaker 0", "Alice", "Unknown Speaker"). Clicking the badge SHALL open an inline input to type a new name or select from existing named speakers.

When the user assigns a name, the frontend SHALL invoke `label_speaker(meeting_id, cluster_label, speaker_name)`, which creates/updates the `speakers` row, links embeddings, and updates all transcript rows for that cluster in the meeting. The original cluster label SHALL be preserved in a `previous_label` column on each transcript row (set only once, on first manual label).

The inline input SHALL show suggestion chips of existing named speakers (excluding auto-generated "Speaker N" labels). Selecting an existing speaker name SHALL merge the current cluster into that speaker — all transcript segments for the cluster are relabeled to the selected name. This is an intentional merge action, not a rename.

Speaker badges SHALL show a small undo icon (visible on hover) for every non-auto-generated label. Clicking the icon SHALL invoke `revert_speaker_label(meeting_id, speaker_label)`, which restores every row carrying that label in the meeting to its pre-label state and unlinks the speaker recognition that produced it. The undo icon SHALL NOT appear on auto-generated labels ("Speaker N") or "Unknown" labels.

`revert_speaker_label` SHALL restore two kinds of label history in one invocation:

1. **Manual rename** (rows with `previous_label IS NOT NULL`): each row SHALL be restored to its own `previous_label`, `speaker_source` set to `NULL`, and `previous_label` cleared to `NULL`.
2. **Recognized name** (rows with `previous_label IS NULL` — diarization matched a stamped speaker and wrote the person's name directly): the system SHALL recover the cluster label the run stored on the matched embedding (`speaker_embeddings.cluster_label`, joined via `speakers.name` for the meeting's embeddings) and relabel those rows to it with `speaker_source = NULL`. If several clusters of the meeting matched the same name (over-split voice), the rows carry no per-cluster marker and SHALL collapse onto the lowest recovered cluster label.

After either path relabels rows, the corresponding embeddings SHALL be unlinked (`speaker_id = NULL`) for the meeting, so future meetings stop recognizing that voice as the named speaker.

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

#### Scenario: Revert a recognized speaker name to its cluster label

- **GIVEN** a meeting where diarization recognized a stamped speaker and wrote "Cynthia Wu" directly onto its rows (`speaker_source = 'auto'`, `previous_label IS NULL`), with the run's embedding row for that cluster storing `cluster_label = "Speaker 1"` and `speaker_id` pointing at the "Cynthia Wu" speaker
- **WHEN** the user hovers over the "Cynthia Wu" badge and clicks the undo icon
- **THEN** all transcript rows with `speaker_label = "Cynthia Wu"` in that meeting revert to `speaker_label = "Speaker 1"`
- **AND** `speaker_source` is set to `NULL`
- **AND** the matched embeddings of the meeting are unlinked (`speaker_id = NULL`), so future meetings stop recognizing that voice as "Cynthia Wu"

#### Scenario: Over-split recognized name collapses onto the lowest cluster label

- **GIVEN** a meeting where two clusters ("Speaker 1" and "Speaker 3") both matched the same stamped speaker "Cynthia Wu", so all their rows carry `speaker_label = "Cynthia Wu"` with no per-cluster marker
- **WHEN** the user reverts "Cynthia Wu"
- **THEN** all those rows relabel to "Speaker 1" (the lowest recovered cluster label)
- **AND** the trade-off is accepted: one person's over-split rows render under one label rather than their pre-recognition split

#### Scenario: Revert handles mixed manual and recognized rows under one name

- **GIVEN** a meeting with recognized "Cynthia Wu" rows (`previous_label IS NULL`) AND rows manually renamed to "Cynthia Wu" (each with its own `previous_label`)
- **WHEN** the user reverts "Cynthia Wu"
- **THEN** the manual rows restore to their own `previous_label` values
- **AND** the recognized rows relabel to the recovered cluster label
- **AND** no row is processed by both paths

#### Scenario: Revert not offered for auto-generated labels

- **GIVEN** a transcript segment with an auto-generated label (`speaker_label = "Speaker 0"`) or an "Unknown" label
- **THEN** the undo icon is not shown on the badge

#### Scenario: Full reset clears previous_label

- **WHEN** the user triggers re-diarization (Speakers button)
- **THEN** all `previous_label` values are cleared along with `speaker_label` and `speaker_source`
