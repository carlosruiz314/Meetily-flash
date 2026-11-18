## ADDED Requirements

### Requirement: Talk-over repair promotion is decode-hardened and ear-gated

A rate-fiction or stutter talk-over span SHALL promote as a repair seed with
decode hardening and an ear-attestation gate: its window SHALL rebuild at the
evidence-backed context width (~9–12s) by the existing repair-window rules,
the separated streams SHALL re-decode under the deterministic profile plus a
no-condition retry when the first pass trips a guard, and the rebuilt window
SHALL route through the live repair-window builder. The retry profile SHALL
be scoped to repair windows: the general transcription path SHALL decode
byte-identically to before this change, pinned by test.

A rebuilt window SHALL splice per-voice rows ONLY when every existing
per-voice condition holds AND the window's walls are present in the
`attestedRepairWindows` app setting — a token-only allowlist of
`{start_s, end_s, attested_date}` entries the user maintains from dated ear
verdicts. An unpinned fired span SHALL keep the mixture render
(byte-identical) and SHALL be recorded in the census as
`awaiting-attestation`. Production code SHALL NOT read the evidence fixture;
the allowlist is its only runtime surface. The census SHALL record every
seed outcome token-only: `fired+attested`, `awaiting-attestation`,
`stood-down:<guard>`, `window-not-built`, `separation-unavailable`.

Before any promotion wiring ships, the candidate census SHALL run on the
reference meeting and the user SHALL review it: every overlap-span candidate
flag recorded token-only with its detector evidence (overlap mass, duration,
floors verdict), so detector precision is measured rather than assumed. The
offline decode-reliability stage SHALL precede wiring: the retry profile
SHALL recover the attested window shapes verbatim on the evidence-home
fixtures before any render changes; failure to recover them is a recorded
STOP for the user to re-plan.

#### Scenario: Attested window renders per-voice after promotion

- **GIVEN** a promoting talk-over span whose window walls match an
  `attestedRepairWindows` entry with a date, and whose rebuilt streams pass
  every per-voice condition
- **WHEN** the rendering regenerates
- **THEN** the window's rows are replaced by the per-voice rows from the
  rebuilt streams
- **AND** the census records `fired+attested` with the seed evidence

#### Scenario: Unpinned fired span keeps the mixture render

- **GIVEN** a promoting talk-over span whose window walls match no
  allowlist entry
- **WHEN** the rendering regenerates
- **THEN** the span renders byte-identical to the mixture render
- **AND** the census records `awaiting-attestation` with the rebuild's
  token-only decode evidence for the attestation clip

#### Scenario: Repair-scoped decode leaves the general path untouched

- **GIVEN** a normal (non-repair) transcription chunk
- **WHEN** the pipeline decodes it
- **THEN** the decode profile, walls, and output are byte-identical to the
  pre-change pipeline

#### Scenario: Retry recovers the attested shapes before wiring ships

- **GIVEN** the three kept window shapes from the archived evidence record
  (validated, mangled/row-removed, control)
- **WHEN** the offline probe runs the widened-window + retry profile against
  the evidence-home attested sequences
- **THEN** each attested shape's rebuilt decode matches its attested
  sequence verbatim
- **AND** a shape that cannot be recovered is a recorded STOP — no wiring
  ships until the user re-plans

#### Scenario: Candidate census precedes render changes

- **GIVEN** the reference meeting's engine output
- **WHEN** the env-gated census pass runs
- **THEN** every overlap-span candidate flag is recorded token-only with
  overlap mass, duration, and floors verdict
- **AND** the recorded census is reviewed by the user before promotion
  wiring lands

### Requirement: Repair-window attestations are auditable

The `attestedRepairWindows` setting SHALL be auditable: every entry SHALL
carry a start wall, an end wall, and a non-empty `attested_date`
(YYYY-MM-DD), and SHALL correspond to a dated ear-truth fixture pin for the
same window (the ear gate cross-checks the two sides: the allowlist may name
only pinned windows, and a per-voice rendered span in the meeting snapshot
must have an allowlist entry). There SHALL be no wildcard entry and no
partial-window match (a splice requires exact wall equality with one
entry). The existing waiver router's hard rules apply unchanged: hard pins
are never waivable and unlabeled waivable pins are never coverable.

#### Scenario: Allowlist entry without a fixture pin fails the gate

- **GIVEN** an `attestedRepairWindows` entry whose walls match no ear-truth
  fixture pin
- **WHEN** the ear gate runs
- **THEN** the gate FAILS, naming the unmatched window

#### Scenario: Per-voice render without an allowlist entry fails the gate

- **GIVEN** a meeting snapshot containing a per-voice rendered span whose
  walls match no allowlist entry
- **WHEN** the ear gate runs
- **THEN** the gate FAILS, naming the span's window

#### Scenario: Exact-wall matching only

- **GIVEN** an allowlist entry whose walls overlap but do not equal a
  rendered span's window
- **WHEN** the splice gate evaluates the span
- **THEN** the span is treated as unpinned and keeps the mixture render

> Both requirements COMPOSE with the existing "Overlap spans render per-voice
> rows from separated streams" requirement (its conditions remain necessary;
> these add the decode-hardening, attestation gate, and census duties). They
> are ADDED rather than MODIFIED so the long canonical text is not restated
> (restating is the transcription-regression failure mode; see the
> decommission-queue-diarization-phase archive note).
