## MODIFIED Requirements

### Requirement: Ear-truth fixture gate validates attribution

A pinned ear-truth fixture (`ear_truth_cde5c264.json`) SHALL hold attribution facts as data — the fixture file is the single source of truth for the entry set (this requirement does not enumerate them) — each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). Entries change only with explicit user confirmation, and two entries (`S3_updates_run`, `S13_userC_to_userB`) SHALL be designated hold-out (not used for any calibration decision).

The fixture SHALL NOT be committed to the public repository: it is local-only golden evidence, under the bright line adopted 2026-09-26 that NOTHING meeting-derived — real, sanitized, or synthetic — enters the repo. Its home resolves through one shared helper: `$MEETILY_LOCAL_EVIDENCE_DIR`, then the default repository sibling `../meetily-flash-local-evidence/` (outside `git clean` reach); a missing fixture means the gate and fixture-lint SKIP LOUDLY, printing the setup instructions — never a silent pass, never a clone-breaking failure. No synthetic or meeting-derived stand-in SHALL be committed in its place: the synthetic subset of the gate is removed outright (a fake golden is no oracle).  

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file — TOKEN-ONLY content — at every verification point, so the acceptance evidence is inspectable without re-running; recorded artifacts and their directory SHALL be gitignored and added to the pre-push guard's protected pathspecs. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented with explicit user sign-off via the fixture's amendment/limitation records); or FAIL (blocks the change). A KNOWN-LIMITATION outcome covers ONLY the failing assertions its amendment record documents: every waivable pin carries a label, every amendment record lists what it covers (`covers` — the entry's waivable pin labels and/or the gate's enumerated failure-kind ids), and a failing assertion whose label or kind no complete amendment record covers SHALL FAIL the gate — a limitation recorded for one defect SHALL NOT absorb failures it does not document, and no wildcard coverage SHALL exist. Hard pins SHALL NOT carry labels and SHALL NOT appear in any covers list: they fail the gate outright regardless of amendment records. A fixture-lint subset SHALL run in plain `cargo test` wherever the fixtures resolve, asserting — for BOTH gate fixtures — every KNOWN-LIMITATION/amendment record carries a user-confirmation date, a reason, and a non-empty covers list whose entries resolve against the record's entry (pin labels present on the entry, or enumerated failure-kind ids).

**Fidelity pins (ear round 2, 2026-10-06)**: needle assertions SHALL be
multiplicity-aware and badge-aware — a pinned phrase SHALL appear exactly
the attested number of times in its window AND under attested badge(s); a
leak fragment of a pinned phrase under an unattested badge, or an extra
occurrence, fails the pin even though the joined window text contains the
phrase. Ear-dictated turn sequences SHALL be recorded as ordered per-row
pin entries in the fixture (badge selector + ordered tokens per expected
row) and enforced as data; a synthesis-stand-down pin kind asserts a span
expected to render mixture rows only (the class-E floors' fixture
expression). This pin schema's normative home is HERE — the
render-fidelity-verification capability references it and owns only the
verification infrastructure. The gate SHALL additionally assert: the
boundary-token leak scan findings (no synth-tail/neighbour-head ≥3-token
chunk among pinned windows), the words-per-second ceiling (no synth row
above the pinned human rate), and parent-link integrity (every
`synth_parent` either a synthesis-atom link or a designed split-piece link;
every synth span's walls overlap its parent row's walls). The duplicate
scan's same-donor exemption remains the attestation of simultaneous speech,
but it SHALL NOT suppress the boundary-token, words-per-second, or
parent-link scans.

The gate's duplicate scan SHALL be provenance-scoped at group level: overlapping-wall turn groups whose absorbed-row id sets intersect (the synthesized stream pair shares the span's source row id — the both-streams-decisive gate is the attestation of simultaneity) SHALL pass the scan as attested simultaneous speech; overlapping-wall groups with disjoint id sets remain double-decode suspects and SHALL still fail the scan. Wall-offsetting (shifting timestamps to dodge the scan) SHALL NOT be used. The gate SHALL log a census over every trigger-fired overlap span — synthesized or not — recording: walls, the span's trigger mass, per-stream best and second-best similarities and margins (including REJECTED spans, so threshold retuning has the near-miss distribution), per-stream pre-normalization span RMS against the clip's RMS, the synthesis outcome with both voice badges, per-stream text ONLY as token-only data (word/character counts and a sha256 of each stream's text), the word-loss diagnostic, the boundary-fragment diagnostic, the words-per-second rate, and the cross-stream duplication diagnostic. DURATION-FLOOR CARVE-OUT (build-stage panel, 2026-10-07): a span stood down by the minimum synthesis duration never reaches separation (the GPU work the floor exists to save), so its vote/RMS near-miss data is not in the census — such spans appear as stand-down records (walls + firing floor + duration) only, and the duration dial is retuned by raising the floor and re-running, not from per-stream margins. Recorded census artifacts SHALL be token-only; verbatim per-stream text SHALL be emitted only to the terminal under an env gate or stored in the private evidence home — never in a repo-committed file. The census is diagnostic and never drops rows. The gate's S16 assertion requires a real loaded Whisper engine in the gate process (the uninitialized-engine degrade would otherwise suppress every synthesized row); the census records the model name used.

#### Scenario: Fixture gate validates the engine before review

- **GIVEN** the meeting audio, local models, and the gate env set
- **WHEN** the gate test runs
- **THEN** every ear-truth entry passes against the derived turns, or each non-passing entry carries a recorded KNOWN-LIMITATION with user sign-off
- **AND** the recorded output file reflects the latest run and contains
  token-only content

#### Scenario: Fixture failure blocks the change

- **GIVEN** an engine change that moves a pinned boundary or flips a pinned label
- **WHEN** the gate test runs
- **THEN** it fails, naming the violated entry
- **AND** the entry may only be resolved by passing the engine or by user-confirmed KNOWN-LIMITATION

#### Scenario: Missing local fixture skips loudly

- **GIVEN** a checkout (fresh clone, CI, or new machine) without `MEETILY_LOCAL_EVIDENCE_DIR` or the sibling default
- **WHEN** the gate or fixture-lint tests run
- **THEN** they skip with a message naming the fixture home convention and setup instructions
- **AND** no synthetic or meeting-derived stand-in runs in their place

#### Scenario: Waivers are auditable

- **GIVEN** a fixture whose KNOWN-LIMITATION list or amendments map contains an entry without a user-confirmation date or reason, or whose amendment record has an empty covers list or names entries that resolve against nothing (a pin label absent from the entry, or an unknown failure-kind id)
- **WHEN** the fixture-lint subset runs in plain `cargo test`
- **THEN** the lint fails, naming the incomplete record

#### Scenario: Uncovered pin failure is not absorbed

- **GIVEN** an entry whose amendment record covers only its badge pins
- **WHEN** a different waivable pin of the same entry fails (a new text pin, for instance)
- **THEN** the gate FAILS naming the uncovered pin — the entry's existing limitation does not absorb it
- **AND** the failure resolves only by passing the engine or by a new user-confirmed amendment record covering that pin

#### Scenario: Hard pins are never waivable

- **GIVEN** a fixture where a hard pin fails inside an entry whose amendment record exists
- **WHEN** the gate evaluates the failure
- **THEN** the gate FAILS outright — hard pins carry no labels, appear in no covers list, and no amendment record absorbs them

#### Scenario: Provenance-scoped duplicate-scan exemption

- **GIVEN** a render containing a synthesized stream pair at overlapping
  walls whose rows share the span's source row id
- **WHEN** the gate's duplicate scan runs
- **THEN** the pair is not reported as a double-decode suspect
- **AND** an overlapping-wall pair tracing to disjoint source ids is still
  reported and fails the gate

#### Scenario: Census carries the retuning evidence, token-only

- **GIVEN** a gate run over a meeting with trigger-fired spans, some
  synthesized and some rejected by the vote margin
- **WHEN** the census is recorded
- **THEN** every trigger-fired span appears with its trigger mass, both
  streams' best/second similarities and margins, and pre-normalization RMS
  ratios — including rejected spans
- **AND** no verbatim meeting text appears in any recorded artifact (text
  is counts + sha256 only)

#### Scenario: Hallucinated synth rows fail the rate pin

- **GIVEN** a meeting render containing a synth row decoding 11 words
  against a 0.37 s wall (~30 words/s, above the pinned human ceiling)
- **WHEN** the gate evaluates the words-per-second pin
- **THEN** the gate fails naming the row — such a decode is machine
  fiction regardless of how fluent it reads

#### Scenario: Leak fragment fails the multiplicity pin

- **GIVEN** a window where the attested phrase renders once under its
  owner's badge and again as a tail fragment inside a synth row under
  another badge
- **WHEN** the gate evaluates the window's multiplicity-aware pin
- **THEN** the gate fails — the phrase occurs more times than attested and
  under an unattested badge
