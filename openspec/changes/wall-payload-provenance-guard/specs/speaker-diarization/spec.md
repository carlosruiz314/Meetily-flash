## ADDED Requirements

### Requirement: Rebuilt-window tails carry mixture-payload provenance

A repair window's synthesized streams SHALL pass a tail-provenance guard
before the window's content hash is computed, before the attestation
verdict evaluates it, and before any splice — all acceptance guards and
verdicts evaluate the post-guard synthesis. The guard walks each stream's
pieces (a stream utterance when `utterances` is non-empty, else the
whole-span text piece) from the LAST token backwards, and trims a
trailing token only when a POSITIVE contradiction holds: the token,
whose proportional wall is its character-fraction position within its
piece span and whose derived interval is spanned by consecutive tokens'
proportional positions, is overlapped — for at least half of that
interval — by a single absorbed-row payload word whose wall width is at
least the census-dialed floor, whose row's badge differs from the
stream's, and which normalizes differently under the frozen comparison
operator (Unicode NFC, lowercase, non-alphanumeric stripped, whitespace
collapsed, exact equality). Cover evaluation is half-open
(`[start_ms, end_ms)`); multi-cover resolves by max interval-overlap,
then the splice's donor row, then earliest start. Tokens at intervals the
payload does not so cover (recovery zones) and same-badge differences
SHALL stay; mid-row tokens SHALL never be examined; absorbed rows whose
wall payload is absent SHALL contribute no walls (per-row fail-open — the
attestation hash/fingerprint verdict remains the fail-closed layer). An
emptied piece SHALL be dropped and the walk SHALL continue into the
previous piece; an emptied stream SHALL read as the absent decode (never
an empty text) and follow the existing empty-decode acceptance gates.
After the guard, each stream's text SHALL equal the join of its pieces'
texts, and the content hash SHALL be computed from that post-guard state,
so a pinned hash is always the hash of the render that ships. The guard
SHALL run for every repair route (legacy phrase-loop, attested-window,
decode-tripped rebuild) through one public pre-pass invoked before
admissibility, and SHALL be structurally unreachable from the normal
transcription chunk path and the mass overlap pre-pass. AT MERGE the
guard SHALL run in census (log-only) mode for every route; trim enactment
on any route SHALL be unlocked only after the env-gated offline census
meets its acceptance bar (would-trims tail-only, predominantly
single-token, zero events inside certified-content regions, placement
error measured below the covering-word scale) on the reference meeting
AND a second meeting, with the user's dated review. A seed whose window
spliced with an enacted trim (or a census-mode would-trim) SHALL
additionally record a token-only evidence note `tail-trimmed:<n>` beside
its outcome token in the seed's census record — the outcome token remains
one of the enumerated outcomes — recorded only for windows whose splice
applied; legacy windows SHALL record the equivalent token-only log line.
Every log line the guard emits SHALL be token-only (indices, wall
positions, counts) and SHALL NOT contain synthesized or mixture row text.

#### Scenario: Contradictory tail token stands down

- **GIVEN** a rebuilt window whose absorbed mixture row carries populated
  per-word walls under badge A, and whose second stream (badge B, B ≠ A)
  decodes a populated-utterance piece whose final token's derived interval
  is majority-covered by one width-above-floor payload word that
  normalizes differently
- **WHEN** the pre-pass runs
- **THEN** the stream's utterance text renders without that trailing
  token, the stream's text field equals the join of its post-trim piece
  texts, and the content hash binds that post-trim state
- **AND** the mid-row tokens of every stream piece are byte-identical to
  the unguarded decode

#### Scenario: Recovery zones, same-voice differences, and sub-threshold overlaps survive

- **GIVEN** a stream's final token whose derived interval is uncovered by
  the payload, or covered only by same-badge words, or majority-covered by
  a below-floor sliver word, or differently-cased/punctuation-carrying
  same-word payload text under a different badge
- **WHEN** the guard runs
- **THEN** the token stays and the decode renders unchanged

#### Scenario: Rows without walls contribute nothing, walled rows still judge

- **GIVEN** a rebuilt window whose absorbed rows mix populated wall
  payloads with absent ones, and a contradicted tail inside a walled
  row's coverage
- **WHEN** the guard runs
- **THEN** the trailing token is trimmed on the strength of the walled row
  alone, and a window whose every absorbed row lacks walls trims nothing

#### Scenario: Piece drop continues the walk; stream empties read as absent

- **GIVEN** a stream whose last piece trims to empty while earlier pieces
  survive, whose previous piece's tail token also contradicts
- **WHEN** the guard runs
- **THEN** the empty piece's row is removed, the walk trims the previous
  piece's tail, and the surviving tokens render byte-identical
- **AND** a stream whose every piece empties reads as the absent decode
  and never replaces non-empty mixture rows

#### Scenario: Pinned hash binds the post-guard render

- **GIVEN** a rebuilt window whose streams the guard trims, and an
  `attestedRepairWindows` entry pinned to the pre-guard content hash
- **WHEN** the attestation verdict evaluates the guarded synthesis
- **THEN** the verdict is `stale-attestation` and the window keeps the
  mixture render byte-identically
- **AND** an entry pinned to the post-guard content hash splices the
  trimmed render, whose seed records the token-only note
  `tail-trimmed:<n>` beside `fired+attested` in its census record

#### Scenario: Census mode at merge; enactment is census-gated

- **GIVEN** a legacy phrase-loop window and a decode-tripped rebuild whose
  streams decode identical contradictory tails, before trim enactment
- **WHEN** the repair path runs at merge
- **THEN** both routes splice their renders byte-identical to the
  unguarded pipeline and both record token-only would-trim evidence
- **AND** no route trims until the census acceptance bar is met on the
  reference meeting and a second named meeting with the user's dated
  review; after enactment both routes splice the trimmed render and
  record `tail-trimmed:<n>`

#### Scenario: Guard logs never carry words

- **GIVEN** any guard decision — trim, would-trim, or no-op
- **WHEN** the guard emits log or census-evidence lines
- **THEN** every line carries indices, wall positions, and counts only —
  no synthesized token, no payload word, no row text

## MODIFIED Requirements

### Requirement: Talk-over repair promotion is decode-hardened and ear-gated

A talk-over span SHALL promote as a repair seed through one of two routes:
a duration-passing span whose separated decode trips rate-fiction or
stutter (decode-tripped), or a trigger-fired span whose walls fall within
an `attestedRepairWindows` entry (attested-window — such a span SHALL
rebuild regardless of the minimum synthesis duration floor, the
attestation being the prior ear evidence). A promoted seed's window SHALL
rebuild by the live repair-window construction (the same builder the
offline probe drives),
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
phrase-loop repairs splice WITHOUT an allowlist entry, through the same
tail-provenance guard as every other route (census-logged before trim
enactment, per the "Rebuilt-window tails carry mixture-payload provenance"
requirement).

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
`window-not-built`, `separation-unavailable`, `rebuild-aborted` — and a
seed whose window the tail-provenance guard trimmed (once enacted) SHALL
additionally record the token-only note `tail-trimmed:<n>` beside its
outcome token. These outcomes extend the existing trigger-fired census
record. A rebuild
panic or join error SHALL be recorded as `rebuild-aborted`, never silently
continued. Union windows re-run the manual-row and synth-atom checks on the
unioned interval; a refused union falls back to the individual windows if
individually clean, else `window-not-built`.

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

#### Scenario: Attested sub-floor span rebuilds despite the duration floor

- **GIVEN** a trigger-fired span of 0.4s wall duration — under the minimum
  synthesis duration floor — whose walls fall within an allowlist entry
  matching on meeting, walls, hash, and fingerprint
- **WHEN** the repair fold evaluates the span
- **THEN** the span's window rebuilds at the attested walls and splices
  per-voice rows from the rebuilt streams
- **AND** the census records `fired+attested`

#### Scenario: Phrase-loop repairs are unaffected by the gate

- **GIVEN** a phrase-loop repair window with no allowlist entry
- **WHEN** the repair fold applies
- **THEN** the phrase-loop repair splices without an allowlist entry
- **AND** its window passes the tail-provenance guard like every route:
  census-logged only before trim enactment, contradictory tails stood down
  after it

#### Scenario: Repair-scoped decode leaves untouched paths byte-identical

- **GIVEN** a normal transcription chunk, and an overlap pre-pass span
  outside any promoted rebuild
- **WHEN** the pipeline decodes them
- **THEN** both paths' decode profile, walls, and output are byte-identical
  to the pre-change pipeline
