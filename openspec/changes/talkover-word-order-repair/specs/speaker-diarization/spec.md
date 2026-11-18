## ADDED Requirements

### Requirement: Talk-over repair promotion is decode-hardened and ear-gated

A rate-fiction or stutter talk-over span SHALL promote as a repair seed with
decode hardening and an ear-attestation gate: its window SHALL rebuild by the
live repair-window construction (the same builder the offline probe drives),
the separated streams SHALL re-decode under the deterministic profile and —
only if the offline lever-ladder measurement shows a retry variant is
required to recover the attested shapes — under that measured minimal retry
variant, and the rebuilt window SHALL route through the live repair-window
builder. The retry variant and its selection rule (pass 1 stands unless it
tripped a guard AND the alternate pass clears every rejection guard; at most
one alternate) SHALL be frozen before any measurement. The retry profile
SHALL be structurally unreachable from the general transcription chunk path
AND from the mass overlap pre-pass: both paths SHALL decode byte-identically
to the pre-change pipeline, pinned by test.

This requirement NARROWS the "Overlap spans render per-voice rows from
separated streams" requirement for rebuilt talk-over windows only. Where a
span's window is rebuilt under this requirement, that requirement's
replacement clause is satisfied only through this attestation gate: its
conditions remain necessary but cease to be sufficient, and a rebuilt window
whose walls match no `attestedRepairWindows` entry SHALL degrade
byte-identically to the mixture render notwithstanding that every existing
condition holds. Windows not rebuilt under this requirement remain governed
by that requirement unchanged. A span that is both gate-decisive under that
requirement and a promotion seed under this one is governed by this
requirement's gate. The gate binds ONLY to the talk-over seed class: live
phrase-loop repairs splice WITHOUT an allowlist entry, unchanged.

A rebuilt window SHALL splice per-voice rows ONLY when every existing
per-voice condition holds AND the window matches an `attestedRepairWindows`
entry on ALL of: same meeting, exact wall equality, equal `render_sha256`,
equal `profile_fp`. A fired window matching no entry, or matching on walls
but not on hash or fingerprint, SHALL keep the mixture render
(byte-identical) — recorded as `awaiting-attestation` or
`stale-attestation` respectively. Production code SHALL NOT read the
evidence fixture; the allowlist is its only runtime surface. The census
SHALL enumerate SEEDS upstream of candidate filtering and record every seed
outcome token-only: `fired+attested`, `awaiting-attestation`,
`stale-attestation`, `stood-down:<guard>` (manual-row-wins mapped here),
`window-not-built`, `separation-unavailable`, `rebuild-aborted`. A rebuild
panic or join error SHALL be recorded as `rebuild-aborted`, never silently
continued. Union windows re-run the manual-row and synth-atom checks on the
unioned interval; a refused union falls back to the individual windows if
individually clean, else `window-not-built`. These outcomes extend the
existing trigger-fired census record.

Before any promotion wiring ships: the candidate census SHALL run on the
reference meeting and the user SHALL review it, producing a dated
token-only review note (counts + ruling) in the change folder; and the
offline lever-ladder probe SHALL recover every attested window shape
verbatim through the FULL builder+splice path, with the comparison operator
frozen before measurement (Unicode NFC, lowercase, punctuation stripped,
whitespace collapsed, exact token-sequence equality). A shape that no leg
recovers is a recorded STOP — no wiring ships until the user re-plans.

**Scenario amendments (apply at archive):** In "Every gate-decisive span is
synthesized — no skip list", the no-curated-list clause and its "ear rulings
… never individual spans" closing are superseded for rebuilt windows, where
the user's dated per-span ear attestation is the acceptance oracle and an
unattested rebuilt window is excluded; the clause continues to govern
windows not rebuilt under this requirement. In "Decisive overlap span
renders two per-voice rows", the GIVEN additionally requires the window's
walls to match an `attestedRepairWindows` entry (for rebuilt windows).

#### Scenario: Attested window renders per-voice after promotion

- **GIVEN** a promoting talk-over span whose window matches an allowlist
  entry on meeting, walls, hash, and fingerprint, and whose rebuilt streams
  pass every per-voice condition
- **WHEN** the rendering regenerates
- **THEN** the window's rows are replaced by the per-voice rows from the
  rebuilt streams
- **AND** the census records `fired+attested` with the seed evidence

#### Scenario: Unpinned fired span keeps the mixture render

- **GIVEN** a promoting talk-over span whose window matches no allowlist
  entry
- **WHEN** the rendering regenerates
- **THEN** the span renders byte-identical to the mixture render
- **AND** the census records `awaiting-attestation` with the rebuild's
  token-only decode evidence for the attestation clip

#### Scenario: Stale pin never splices a never-heard decode

- **GIVEN** a promoting talk-over span whose window walls match an allowlist
  entry, but whose rebuilt decode's content hash or decode-profile
  fingerprint differs from the entry's
- **WHEN** the rendering regenerates
- **THEN** the span renders byte-identical to the mixture render
- **AND** the census records `stale-attestation` with both hashes logged

#### Scenario: Attested window that trips a guard still stands down

- **GIVEN** a promoting talk-over span whose window matches a valid
  allowlist entry, and whose rebuilt stream trips a rejection guard
- **WHEN** the rendering regenerates
- **THEN** the span renders byte-identical to the mixture render
- **AND** the census records `stood-down:<guard>`

#### Scenario: Phrase-loop repairs are unaffected by the gate

- **GIVEN** a phrase-loop repair window with no allowlist entry
- **WHEN** the repair fold applies
- **THEN** the phrase-loop repair splices as it does today

#### Scenario: Repair-scoped decode leaves untouched paths byte-identical

- **GIVEN** a normal transcription chunk, and an overlap pre-pass span
  outside any promoted rebuild
- **WHEN** the pipeline decodes them
- **THEN** both paths' decode profile, walls, and output are byte-identical
  to the pre-change pipeline

#### Scenario: Retry recovers the attested shapes before wiring ships

- **GIVEN** the three kept window shapes from the archived evidence record
  (validated, mangled/row-removed, control)
- **WHEN** the offline lever-ladder probe drives the full builder+splice
  path against the evidence-home attested sequences under the frozen
  comparison operator
- **THEN** each attested shape's rebuilt render matches its attested
  sequence verbatim on the measured minimal leg
- **AND** a shape that no leg recovers is a recorded STOP — no wiring ships
  until the user re-plans

#### Scenario: Candidate census precedes render changes

- **GIVEN** the reference meeting's engine output
- **WHEN** the env-gated census pass runs
- **THEN** every overlap-span candidate flag is recorded token-only with
  overlap mass, duration, and floors verdict, enumerated independent of
  enrollment
- **AND** the recorded census is reviewed by the user before promotion
  wiring lands, producing the dated review note

#### Scenario: Rebuild abort is recorded, not swallowed

- **GIVEN** a promoted window whose rebuild panics or fails to join
- **WHEN** the repair pass handles the seed
- **THEN** the census records `rebuild-aborted` for that seed
- **AND** the span renders byte-identical to the mixture render

#### Scenario: Empty stream decode never replaces non-empty mixture rows

- **GIVEN** a promoted window whose rebuilt stream decodes to empty text
  while the span's mixture rows are non-empty
- **WHEN** the splice evaluates the window
- **THEN** the window stands down and renders byte-identical to the mixture
  render

### Requirement: Repair-window attestations are auditable

The `attestedRepairWindows` setting SHALL be auditable. Each entry SHALL
carry exactly `{meeting_id, start_s, end_s, attested_date, render_sha256,
profile_fp}` — no additional field SHALL ever be added to the setting or its
entries (pinned by a `deny_unknown_fields` deserialization test) — and the
setting SHALL live in the app's local Tauri store and SHALL never be
exported, synced, or committed. `attested_date` SHALL be a non-empty
YYYY-MM-DD date; a malformed date fails the fixture lint, naming the entry.
Entries SHALL be unique by (meeting, walls); a duplicate pair fails the
fixture lint. A malformed or unparseable setting SHALL be treated as empty
for that run (fail-closed) with a loud warning.

Each entry SHALL correspond to a dated ear-truth fixture pin of kind
`repair_attestation` whose params equal the entry (meeting, walls,
`attested_date`, `render_sha256`, `profile_fp`); the fixture lint asserts
the date parses as YYYY-MM-DD. The cross-check runs ONLY test-side — under
`cargo test` via the evidence-home fixture loader with its loud skip when
absent; production code SHALL NOT read the evidence fixture. The allowlist
may name only pinned windows, and every per-voice rendered span FROM A
PROMOTED REBUILD (identified via the census record) SHALL have an allowlist
entry. There SHALL be no wildcard entry and no partial-window match (a
splice requires exact wall equality with one entry, within the same
meeting). Each attestation round SHALL produce a before/after clip and a
token-only manifest per fired span, written only under the evidence home or
the meeting folder. The existing waiver router's hard rules apply unchanged:
hard pins are never waivable and unlabeled waivable pins are never
coverable.

#### Scenario: Allowlist entry without a fixture pin fails the gate

- **GIVEN** an `attestedRepairWindows` entry whose (meeting, walls) match no
  ear-truth fixture pin
- **WHEN** the ear gate runs
- **THEN** the gate FAILS, naming the unmatched window

#### Scenario: Promoted per-voice render without an allowlist entry fails the gate

- **GIVEN** a meeting snapshot containing a per-voice rendered span from a
  promoted rebuild whose walls match no allowlist entry
- **WHEN** the ear gate runs
- **THEN** the gate FAILS, naming the span's window

#### Scenario: Hash or fingerprint mismatch fails the cross-check

- **GIVEN** an allowlist entry whose walls match a fixture pin but whose
  `render_sha256` or `profile_fp` differs from the pin's params
- **WHEN** the ear gate runs
- **THEN** the gate FAILS, naming the stale entry

#### Scenario: Exact-wall matching only, within the same meeting

- **GIVEN** an allowlist entry whose walls overlap but do not equal a
  rendered span's window, or whose meeting differs from the span's meeting
- **WHEN** the splice gate evaluates the span
- **THEN** the span is treated as unpinned and keeps the mixture render

#### Scenario: Duplicate entries and malformed dates fail the lint

- **GIVEN** two allowlist entries with the same meeting and walls, or an
  entry whose `attested_date` is not YYYY-MM-DD
- **WHEN** the fixture lint runs
- **THEN** the lint FAILS, naming the offending entries

#### Scenario: Unknown fields are refused, not ignored

- **GIVEN** an allowlist entry carrying a field beyond the six allowed
- **WHEN** the setting deserializes
- **THEN** deserialization fails and the setting is treated as empty with a
  loud warning
