## ADDED Requirements

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
