## ADDED Requirements

### Requirement: Render shape is verifiable in milliseconds without GPU

The repository SHALL contain a census-replay harness runnable in plain
`cargo test` (no meeting audio, no models, no GPU) that drives the REAL
pure splice functions (`apply_overlap_synthesis`, `apply_loop_repairs`,
`decode_stream_utterances` with a fake decoder) over synthetic, token-only
census fixtures. A fixture SHALL pin: the full `StreamEvidence` inputs
(spans, clusters, margins, rms ratios, covered atoms, utterance walls),
fake stream texts engineered for specific defect shapes (fragment tails,
word deficits, stutter, punctuation-only abstentions, impossible rates),
and the fake-decoder contract keyed by (span, stream). The harness SHALL
NOT re-implement any splice logic — it calls the production functions — and
SHALL carry no verbatim meeting text (synthetic text only; the live gate
remains the real-model parity pin). The fast-fail ladder SHALL be: unit →
warm replay → span probe → cold gate; a cold gate run SHALL NOT be spent
until the replay tier reproduces the target shape.

#### Scenario: A decode-input change fails the replay tier in seconds

- **GIVEN** a census fixture pinning the fake-decoder contract and stream
  inputs for a synthesized span
- **WHEN** the decode path changes the bytes handed to the decoder (e.g.,
  trims, pads, or re-slices the stream)
- **THEN** the harness fails in plain `cargo test` naming the changed
  contract — without any GPU run

#### Scenario: Word scatter fails conservation at the replay tier

- **GIVEN** a fixture whose absorbed mixture rows hold a known word
  multiset
- **WHEN** the splice scatters or drops words into the replacement rows
- **THEN** the harness's word-conservation property fails naming the span

#### Scenario: The harness runs green on the pinned shapes

- **GIVEN** the committed census fixtures
- **WHEN** `cargo test` runs the harness
- **THEN** every fixture replays through the production splice functions
  to the pinned row shapes in under a few seconds total

### Requirement: Fidelity scans are shared, not gate-private

The fidelity scans SHALL be pure functions in the synthesis domain, called
by BOTH the production census and the ear-truth gate: boundary-token
overlap (synth row tail vs temporal neighbour head), words-per-second
against row walls, and parent-link integrity (dangling `synth_parent`
semantics, wall containment). A scan existing only in the gate (production
never sees it) or only in production (the gate cannot assert it) SHALL be
considered a divergence defect. Scan outputs are diagnostics: they SHALL be
recorded in the census (token-only) and asserted by the gate where a pin
exists; no scan SHALL drop or revert a rendered row by itself — enforcement
happens only through gate pins or ear-ruled acceptance-gate graduation.

#### Scenario: Both surfaces see the same scan outcome

- **GIVEN** a render containing a synth row whose tail shares ≥3 normalized
  tokens with the head of its temporal neighbour
- **WHEN** a Speakers run records its census AND the gate replays the same
  fixture
- **THEN** both surfaces report the identical boundary-token finding

#### Scenario: Both census and gate name the same offender

- **GIVEN** a render whose parent-link scan finds a synth span whose walls
  do not overlap its parent row's walls
- **WHEN** the production census is recorded AND the gate replays the same
  render
- **THEN** both artifacts name the same span and the same violated link

### Requirement: Ear pins are fixture data, not code

Ear-dictated turn sequences SHALL be expressible as ordered per-row pin
entries in the ear-truth fixture (badge selector + ordered tokens per
expected row, plus a synthesis-stand-down kind asserting a span renders
mixture rows only), so a dictation lands as data without code changes.
This requirement's NORMATIVE home is the speaker-diarization delta's
fidelity-pins paragraph on "Ear-truth fixture gate validates attribution" —
that requirement owns the pin schema and its enforcement; this capability
owns the verification infrastructure (harness + shared scans) and SHALL NOT
duplicate the pin semantics (a threshold amendment edits one spec). The
hand-coded S16/S18 assertion blocks migrate onto this schema.

#### Scenario: A dictation lands as a fixture entry

- **GIVEN** the user dictates a turn sequence for a window
- **WHEN** the dictation is transcribed into the fixture as an ordered
  per-row pin
- **THEN** the gate enforces it with no Rust changes
- **AND** the pin's failure output names the offending row and the pin id
