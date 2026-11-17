## ADDED Requirements

### Requirement: Scramble-seed promotion applies only through the net-worse guard

The run SHALL detect scramble seeds — overlap spans that reached separation
(the duration-floor carve-out never separates and is out of scope), whose
FINAL synthesis outcome is the full mixture render (a span partially
synthesized with a surviving stream has no pure mixture baseline and is out
of scope), and whose separated streams' Whisper decode exceeds the pinned
human speech ceiling (the interleave fingerprint: two voices' interleaved
words decoded into one stream). The fingerprint decode is evidence only:
its text is NEVER rendered. Each seed SHALL promote its containing window
to the separated-repair pipeline; the window is built by the same
repair-window rules (edge-row absorption included). Two seeds in one window
SHALL resolve to one union window with one guard verdict. Promotions rank
in the repair pipeline's existing seed priority (stutter, then loop, then
scramble — dedupe-to-first order is load-bearing and pinned by test), enter
as a third seed source in the post-duplicate-resolution repair fold, and
ONLY promoted windows route through the net-worse guard (a seed-kind
discriminator keeps stutter/loop windows on today's ungated behavior).

The net-worse guard SHALL evaluate the repair render against the replaced
mixture rows BEFORE any replacement, under the principle that the repair
may only change what is broken: text the promotion was not asked to fix —
tokens outside the seed span's walls — must survive. The conservation set
is scoped on the BASELINE side (repair-side tokens carry no per-token
timestamps): a baseline row fully outside the seed span is conserved whole;
a row straddling the span walls is partitioned proportionally by word count
at the wall (the replacement-scope precedent), its outside piece conserved.
Alignment SHALL be per badge: baseline conservation pieces are grouped by
their rendered badge, and each group's ordered tokens are aligned (LCS over
normalization-stripped tokens — case, punctuation, whitespace) against the
stream whose vote evidence resolves to that badge. A global alignment of
the concatenated streams is not acceptable: interleaved speech caps it near
half even for a perfect repair. The ratio is RECALL (LCS length over the
group's token count) so repair-side tokens outside every group — seed-span
words, unscopable without timestamps — cost nothing; precision SHALL be
recorded alongside recall. An EMPTY conservation set SHALL NOT promote (an
unanchored full-window rewrite has no net-worse evidence). Per-badge
alignment SHALL catch a badge flip on conserved text: the conserved tokens
land in the wrong alignment and both groups degrade.

The repair render SHALL pass only when every conservation group survives
under the pinned conservation bar, whose value SHALL be settled by an
offline evidence pass over the attested window shapes (the validated shape,
the mangled shape, and the 12s control) BEFORE any production threshold
exists: the statistic (denominator, normalization, minimum-set handling) is
frozen before numbers are recorded, the validated and mangled ratios must
separate by a mechanically pinned margin, and the 12s control's expected
verdict is pinned in the same settlement. The bar SHALL be calibratable
only under the ear-truth fixture gate's governance, hold-out entries
excluded. If the offline pass proves the validated render hard-conserves
its non-seed interior, the guard SHALL use that hard interior invariant in
place of a ratio; if hard conservation fails, the bar SHALL be user-ruled
as a class dial — an implementer-chosen midpoint does NOT ship. A
window-edge exemption band SHALL exist only if the same offline pass pinned
it; absent that evidence the guard conserves the full non-seed scope,
absorbed edge rows included. The guard SHALL NOT read ear-truth fixtures or
any meeting-derived fixture: its only baseline is the meeting's own mixture
rows.

Promoted-window rows SHALL carry the persisted hard invariants: `original_id`
inheritance and `synth_parent` links per the replacement-scope geometry, the
engine-emitted continuation fact, and synthesis-atom status (the same-speaker
merge and later consolidation SHALL NOT re-merge them). Promoted rows SHALL
pass the hallucination guards (non-empty after trimming, not quarantined,
degenerate-repeat and echo checks) and the words-per-second ceiling AT THE
GRANULARITY ACTUALLY RENDERED — the whole-window decode re-granularizes the
seed span's rate check, and the rendered rows are held to the same ceiling as
any synth row, so the phantom-short-utterance class cannot re-enter through
promotion.

The promotion SHALL stand down — the window renders byte-identical to the
mixture render — when the guard rejects the repair, when the guard errors,
when the baseline rows are missing, or when the conservation set is empty;
the degrade is fail-closed. A window containing a surviving manually-labeled
row SHALL NOT promote (manual rows outrank the repair), and a window
overlapping a span the floors already synthesized SHALL NOT promote
(re-decoding attested-good stream rows is risk with no upside). The
PRODUCTION regeneration census SHALL record, for EVERY seed fire, an
outcome — promotion applied, guard stand-down, window not built (over-cap,
no donor, deduped), separation unavailable — token-only: walls, verdict,
alignment ratio, per-badge token counts + sha256, and badge resolution
(verbatim text only under an env gate or in the private evidence home), so
a mis-tuned bar is visible and correctable, never silent. Seed-decode
evidence SHALL cache in the meeting folder — outside the repository — keyed
by window walls + absorbed-row id set + model versions, written atomically;
an incomplete, corrupt, or key-changed entry SHALL re-decode; the cache
SHALL hold decode evidence only (never guard verdicts, never a text field),
and a test SHALL verify its resolved root lies outside the repository.

#### Scenario: Seed promotes and the repair applies

- **GIVEN** a stood-down overlap span whose separated decode exceeds the
  pinned human speech ceiling
- **WHEN** its containing window's repair render conserves every
  per-badge conservation group under the pinned conservation bar
- **THEN** the window renders the repair rows in place of the mixture rows
- **AND** the census records the promotion with recall, precision, and both
  streams' token counts

#### Scenario: Guard stands down a fluent-substitution mangle

- **GIVEN** a promoted window whose repair render substitutes fluent but
  different tokens for already-correct non-seed-span text, where every
  existing signal (stutter, rate ceiling, margin, collapse, word-loss)
  passes
- **WHEN** the net-worse guard evaluates the repair render
- **THEN** the promotion stands down and the window renders byte-identical
  to the mixture render
- **AND** the census records the stand-down with the ratio and the
  diverging token counts

#### Scenario: Word-order scramble fails the ordered statistic

- **GIVEN** a repair render whose conservation-group tokens match the
  baseline as a multiset but scramble a single-voice region's word order
  below the pinned bar
- **WHEN** the guard evaluates the repair render
- **THEN** the promotion stands down — the ordered alignment statistic
  rejects what a multiset comparison would pass

#### Scenario: Per-badge alignment survives interleaved speech

- **GIVEN** a conservation set whose baseline rows alternate badges in
  time order while the repair render is two whole-window stream rows
- **WHEN** the guard aligns each badge group against its vote-resolved
  stream
- **THEN** a perfect separated repair scores near 1.0 per group — global
  alignment of the concatenated streams would cap near 0.5 and MUST NOT be
  used

#### Scenario: Badge flip on conserved text fails the guard

- **GIVEN** a promoted window whose repair render carries the conserved
  tokens but resolves them to the wrong streams
- **WHEN** the per-badge alignment runs
- **THEN** both groups degrade below the bar and the promotion stands down
- **AND** the census records the badge resolution per stream

#### Scenario: Empty conservation set does not promote

- **GIVEN** a seed whose containing window's baseline rows all lie inside
  the seed span's walls
- **WHEN** the promotion evaluates the window
- **THEN** the window does not promote — an unanchored full-window rewrite
  has no net-worse evidence
- **AND** the census records the stand-down with the empty-set reason

#### Scenario: Two seeds resolve to one union window

- **GIVEN** two seed spans whose containing windows overlap
- **WHEN** the promotion builds windows
- **THEN** one union window with one guard verdict results, in deterministic
  order — no double promotion

#### Scenario: Manual row blocks promotion

- **GIVEN** a seed whose containing window includes a surviving
  manually-labeled row
- **WHEN** the promotion evaluates the window
- **THEN** the window does not promote — the manual row survives as today

#### Scenario: Floors-synthesized region blocks promotion

- **GIVEN** a seed whose containing window overlaps a span the floors
  already synthesized into attested per-voice rows
- **WHEN** the promotion evaluates the window
- **THEN** the window does not promote — the synthesized rows are not
  re-decoded

#### Scenario: Guard error fails closed

- **GIVEN** a promoted window whose baseline rows are missing or whose
  guard evaluation errors
- **WHEN** the promotion would replace the mixture rows
- **THEN** the window renders byte-identical to the mixture render — a
  guard failure can never apply an unverified repair

#### Scenario: Window that never builds is censused

- **GIVEN** a seed fire whose window exceeds the repair-chain cap, finds no
  donor, is deduped away, or whose separation is unavailable
- **WHEN** the production census is recorded
- **THEN** the seed fire appears with its outcome (window not built /
  separation unavailable) — no seed fire is silent

#### Scenario: Promoted rows pass the ceiling at rendered granularity

- **GIVEN** a promoted window whose repair render splits into a
  short utterance decoding above the pinned human speech ceiling
- **WHEN** the synthesis acceptance evaluates the rendered rows
- **THEN** the offending row degrades as any synth row does — promotion
  does not bypass the words-per-second ceiling or the hallucination guards

#### Scenario: Cache key change re-decodes

- **GIVEN** a cached seed decode whose window walls, absorbed-row id set,
  or model versions differ from the current evaluation
- **WHEN** the seed census runs
- **THEN** the stale entry re-decodes — as does an incomplete or corrupt
  one — and the cache root is verified to lie outside the repository

#### Scenario: Census verdicts are token-only

- **GIVEN** a run where seeds fired, some promotions applied and some
  stood down
- **WHEN** the census is recorded
- **THEN** every seed fire carries its window walls, outcome, ratio, and
  per-badge token counts + sha256
- **AND** no verbatim meeting text appears in any recorded artifact

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
byte-identical to the mixture render, UNLESS the span seeds a promoted
window under "Scramble-seed promotion applies only through the net-worse
guard" (the seed's fingerprint decode is evidence only and its text is
never rendered); the real words already live in the mixture rows, so no
attested text is lost by the degrade.

If any condition fails — vote margin miss on either stream, both streams
resolving to the same badge, empty or quarantined stream text, pre-
normalization RMS below the eligibility floor, span duration below the
minimum synthesis duration, a stream decode exceeding the words-per-second
ceiling, separation model missing,
transcription engine unavailable, or the span containing a surviving
manually-labeled row (the manual row wins; synthesis degrades for that
span) — the rendering SHALL be byte-identical to today's mixture render,
except where the failed span seeds a promoted window under "Scramble-seed
promotion applies only through the net-worse guard", whose own guard
governs any replacement.
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
- **AND** the rejected decode may additionally seed a promoted window under
  the scramble-seed promotion requirement — the seed's evidence decode is
  itself never rendered

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
