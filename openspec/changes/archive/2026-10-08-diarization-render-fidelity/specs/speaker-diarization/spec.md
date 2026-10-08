## MODIFIED Requirements

### Requirement: Overlap spans render per-voice rows from separated streams

The pipeline SHALL transcribe each separated stream (span carved with
context padding, RMS-normalized) through the loaded Whisper engine when
the separation pre-pass identifies an overlap span and the margin-gated
voice votes are decisive for BOTH separated streams (one vote each, at or
above margin), and the span's mixture rows in the regenerated rendering
SHALL be replaced by one row per decisive stream — span walls, the
stream's voice badge, the stream's text. The two streams' decisive votes SHALL resolve to
DIFFERENT speaker clusters; a same-badge outcome (separation collapse) SHALL
degrade to the mixture render. Vote evidence SHALL carry stream identity (a
per-span record of stream → cluster, margin, text) so badges are not derived
from vector order. The two stream rows SHALL render in a deterministic
order pinned by test.

Each stream row SHALL inherit the span's source row id as `original_id` so
persistence and undo flows can trace it (a rendering row whose
`original_id` is absent from `transcript_sources` is silently dropped at
persist). When a span's covered atoms intersect multiple source rows, each
stream row SHALL inherit the id of the source row with maximum covered-atom
overlap (tie: earliest start); the selection SHALL be pinned by test.
Replacement scope (USER-RULED 2026-09-30, after the live gate proved the
whole-row rule never fires): a mixture row the span FULLY covers is
replaced whole; a row the span PARTIALLY covers is SPLIT at the span walls
— its pre-span words persist as a head piece, its post-span words as a tail
piece (word partition proportional to duration), and the two stream rows
sit between them. No word is ever lost. Every piece born from a split
SHALL carry a persisted `synth_parent` link (its own source row id; stream
rows: the donor's) so downstream analysis — the summarizer above all — can
reconstruct the pre-split row by grouping on it. Head/tail pieces are
ordinary merge-eligible speech (not synthesis atoms); the stream rows are
synthesis atoms as before.

The stream decode SHALL use the meeting's resolved language — the concrete
code when the user preference names one; automatic preference states SHALL
degrade (no synthesis), because the automatic resolution is translation and
stream rows translated to English would be the defect. Never
`auto-translate`, never per-stream auto-detection. Streams SHALL decode under a deterministic
profile (pinned language, greedy search, temperature 0) rather than the
live chunked profile, and the profile SHALL NOT consume token timestamps
(stream rows use span walls). A stream SHALL be eligible for synthesis only
if its PRE-normalization span RMS is at least a stated fraction of the
mixture clip's RMS (recorded in the census); the post-normalization RMS
floor alone is not eligibility evidence, because normalization amplifies
near-silence into the hallucination regime.

The synthesis pass SHALL run after duplicate-cluster resolution — a
same-span stream pair SHALL never be classified as a re-transcription
cluster and dropped. Stream text is untrusted model output: it SHALL pass
the hallucination guards (non-empty after trimming, not quarantined,
degenerate-repeat and echo checks) exactly as live transcription output
does.

**Synthesis rate floor (ear round 2, 2026-10-06)**: a span whose wall
duration is below the pinned minimum synthesis duration SHALL NOT
synthesize — its streams cannot carry a decodable utterance, and the
separated decode of such a span is the hallucination regime (six meeting
spans decoded fluent template sentences at 14–35 words/s against 0.37–1.03s
walls). As a backstop independent of walls, a stream decode whose
words-per-second rate against its own span exceeds the pinned human speech
ceiling SHALL be rejected as hallucinated and degrade its stream to no
text. When the stream split into utterances, the same ceiling SHALL also
apply at the granularity actually rendered: any stream utterance whose
words-per-second rate against its own utterance walls exceeds the ceiling
SHALL reject the synthesis (clip-09 ear ruling 2026-10-07: the phantom
"You don't?" row was 2 words in a 0.2s utterance — 10 w/s, invisible to
the span-level check; ordinary cross-stream phrase echo is real speech and
MUST NOT gate — two dial shapes keyed on window geometry or text overlap
were tried for that ruling and reverted when each broke an attested
render). Both floors are general dials recorded in the census — never a
per-span skip list. A span stood down by either floor SHALL render
byte-identical to the mixture render; the real words already live in the
mixture rows, so no attested text is lost by the degrade.

If any condition fails — vote margin miss on either stream, both streams
resolving to the same badge, empty or quarantined stream text, pre-
normalization RMS below the eligibility floor, span duration below the
minimum synthesis duration, a stream decode exceeding the words-per-second
ceiling, separation model missing,
transcription engine unavailable, or the span containing a surviving
manually-labeled row (the manual row wins; synthesis degrades for that
span) — the rendering SHALL be byte-identical to today's mixture render.
`transcript_sources` SHALL remain byte-identical across the whole flow.

Stream rows SHALL comply with the persisted hard invariants: each carries
the continuation fact (`continues_previous = true` when the stream's text
begins mid-sentence), and a stream row is a synthesis atom — the same-
speaker merge and any later consolidation SHALL NOT re-merge it into an
adjacent same-speaker turn. A word-loss diagnostic SHALL be computed per
synthesized span — duration-normalized rates (words per second over the
span, flagged in both directions: too low suggests lost words, too high
suggests hallucination) with the raw-count comparison against the SUM of
all replaced mixture rows as a secondary number — and a cross-stream
duplication diagnostic SHALL flag stream pairs whose texts share a
contiguous ≥3-token chunk covering ≥80% of the shorter (suspected separator
leak). A boundary-fragment diagnostic SHALL additionally flag — provenance-
scoped — a synth row whose tail tokens share a contiguous ≥3-token chunk
with the HEAD tokens of its temporal neighbour row (or vice versa),
regardless of the 80% coverage bar: boundary fragments of long rows are the
leak signature the coverage bar structurally misses. All diagnostics are
flag-only: they SHALL NOT drop or revert any row.

#### Scenario: Decisive overlap span renders two per-voice rows

- **GIVEN** an overlap span where both separated streams have
  margin-passing voice votes for DIFFERENT clusters and guard-clean text
- **WHEN** the rendering regenerates
- **THEN** the span's mixture rows are replaced by two rows — one per
  stream — with span walls, each stream's voice badge, and a deterministic
  order
- **AND** each stream row's `original_id` is the pinned source-row
  selection for that span

#### Scenario: Undecidable span degrades to the mixture render

- **GIVEN** an overlap span where a vote margin misses, both streams vote
  the same cluster, a stream's text is empty (including whitespace- or
  punctuation-only) or quarantined, a stream's pre-normalization RMS is
  below the eligibility floor, the separation model is missing, the
  transcription engine is unavailable, the span is clamped at a recording
  edge, or a manually-labeled row survives inside the span
- **WHEN** the rendering regenerates
- **THEN** the span renders exactly today's mixture rows, byte-identical

#### Scenario: Sub-second span does not synthesize

- **GIVEN** an overlap span whose wall duration is below the pinned
  minimum synthesis duration
- **WHEN** the rendering regenerates
- **THEN** the span renders exactly today's mixture rows, byte-identical
- **AND** the census records the stand-down with the floor that fired

#### Scenario: Impossible-rate decode is rejected as hallucinated

- **GIVEN** a span whose stream decode produces text at a words-per-second
  rate above the pinned human speech ceiling against its span duration
- **WHEN** the synthesis acceptance evaluates the stream
- **THEN** that stream degrades to no text and the span renders
  byte-identical to the mixture render (or per-voice with only the
  surviving stream, if the other stream passes)
- **AND** the census records the rejected decode with its measured rate

#### Scenario: Immutability and persist round-trip

- **GIVEN** a full diarization run that synthesized at least one overlap
  span
- **WHEN** the render persists and is refetched
- **THEN** `transcript_sources` is byte-identical to before the run
- **AND** the persisted stream rows keep their per-voice badges and text

#### Scenario: Word-loss and cross-stream flags are diagnostic only

- **GIVEN** a synthesized span whose stream rows lose words vs the replaced
  mixture rows, or whose two stream texts are near-identical
- **WHEN** the render and the census are produced
- **THEN** the span is flagged in the census for the respective diagnostic
- **AND** the span still renders its synthesized rows — nothing is dropped
  or reverted by a flag

#### Scenario: Boundary leak fragment is flagged across the coverage bar

- **GIVEN** a synth row whose tail shares a contiguous ≥3-token chunk with
  the head of its temporal neighbour row, where the chunk covers far less
  than 80% of the shorter row
- **WHEN** the boundary-fragment diagnostic runs over the render
- **THEN** the pair is flagged in the census with both rows' ids and the
  shared chunk length
- **AND** no row is dropped — the flag is evidence for the gate pins

#### Scenario: Stream pair survives duplicate-cluster resolution

- **GIVEN** a same-span stream pair whose texts are token-similar (an echo
  or a genuine repeat)
- **WHEN** the duplicate-cluster resolver runs
- **THEN** both stream rows survive — no voice vanishes

#### Scenario: Every gate-decisive span is synthesized — no skip list

- **GIVEN** the set of overlap spans meeting the both-streams-decisive gate
  across a meeting
- **WHEN** the rendering regenerates
- **THEN** every span in that set is synthesized — no span is excluded by
  any curated list; ear rulings tune the general thresholds, never
  individual spans

#### Scenario: Manual row wins over synthesis

- **GIVEN** a span where the user manually labeled a row in a previous run
- **WHEN** the next diarization run regenerates the render
- **THEN** the span does not synthesize (the manual row survives as today)
- **AND** the other voice's fresh row is not suppressed by midpoint
  suppression

### Requirement: Ear-truth fixture gate validates attribution

The repository SHALL contain a pinned ear-truth fixture (`frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json`) holding attribution facts as data, each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). The 13 recorded entries (user-ear answers of 2026-09-04, verbatim in the local fixture's answers record, resolved to absolute times via recovered clip offsets): single-voice spans ≈5.9–12.8 (the user's sentence), 15.5–20.8, 24.5–29.5 (UserB), 32.0–38.0, 2803–2820 (UserC); voice changes at ≈13.0 (user→UserB), ≈29.5 (UserB→user), ≈31.5 (user→UserB one-word backchannel), ≈38.0/≈39.0 ("okay" interjection), ≈2776.4 (two voices trading), ≈2803.0 (UserB→UserC), ≈2821.0 (UserC→UserB), and the 02:12–02:50s anchor `voice_change_at` ≈161s ±0.75 with the pinned mid-sentence tail on the earlier side (verbatim tails live only in the local fixture). Entries change only with explicit user confirmation, and two entries (`S3_updates_run`, `S13_userC_to_userB`) SHALL be designated hold-out (not used for any calibration decision). [Restatement note: two needle descriptions that were verbatim in the live text are tokenized here under the PII bright line; the live-spec scrub is tracked as a separate change.]

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file — TOKEN-ONLY content — at every verification point, so the acceptance evidence is inspectable without re-running; recorded artifacts and their directory SHALL be gitignored and added to the pre-push guard's protected pathspecs. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented in this change with explicit user sign-off); or FAIL (blocks the change). A synthetic subset of the gate (the frame/split/attachment rules on recorded fixture arrays) SHALL run in plain `cargo test` without audio or models.

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

#### Scenario: Synthetic gate subset runs in CI

- **GIVEN** a plain `cargo test` without meeting audio or models
- **WHEN** the gate's synthetic subset runs
- **THEN** the run/split/attachment rules are asserted against recorded fixture arrays without env gates

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

## ADDED Requirements

### Requirement: Repair windows absorb the edge row whole

A repair window whose chain reaches a cap-straddling row SHALL absorb that
row WHOLE (extending the window to the row's walls) when
the extended window stays within the separate repair-chain cap, which SHALL
be its own constant — distinct from the mass-span guard — so raising it
never weakens mass-span eligibility. Partial edge coverage SHALL NOT be
introduced: partially-covered rows' words resurface inside the stream
decodes and render twice (the documented trap). The clip-03 defect class
(a two-voice plain row just past a repair window's end wall carrying
wrong-badge head words) is the motivating case: the chain stopped at the
shared mass-span cap, leaving the edge row unrepaired.

#### Scenario: Edge row is absorbed whole

- **GIVEN** a repair window whose end wall sits mid-row of a neighbouring
  two-voice row, with the extension within the repair-chain cap
- **WHEN** the repair windows are built
- **THEN** the window extends to the edge row's walls and the edge row is
  absorbed whole into the re-synthesis
- **AND** the mass-span eligibility guard is unchanged

#### Scenario: Cap-exceeding edges stop the chain, never slice

- **GIVEN** a repair chain whose edge-row absorption would exceed the
  repair-chain cap
- **WHEN** the repair windows are built
- **THEN** the chain stops before the edge row — the row keeps its old
  shape, and no head/tail slice of it enters the stream decodes
