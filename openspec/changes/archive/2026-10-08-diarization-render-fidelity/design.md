## Context

**Archive-order dependency**: this change assumes `overlap-stream-
retranscription` archives FIRST — its MODIFIED requirement blocks were
copied from that change's in-flight delta and must be re-derived from the
live spec immediately before this change's own archive.

Ear round 2 (2026-10-06) ruled four windows and exposed four defect classes the
ear-truth gate structurally cannot see; the explore-stage adversarial panel
(5 reviewers — taxonomy, dials, architecture, process, mining) then collapsed
the taxonomy to two root axes (decode CONTENT vs splice GEOMETRY), found
class E (physically impossible decodes: Whisper hallucinating fluent template
sentences on sub-second separated streams — six meeting windows, up to ~35
words/s), and measured every class meeting-wide from existing data. The
process reviewer named the whack-a-mole root cause: the gate validates
synthesis decisions (presence-only, order-free, badge-blind needles) not
render fidelity, and every fix raised trigger recall while the splice shape
stayed lossy. Three gate runs (~70 min each) were burned in one day catching
splice-design errors the proposed replay tier would have caught in seconds.

Constraints pinned by prior changes and panel verdicts:
- DECODE INPUTS NEVER CHANGE (decode once per stream, full samples;
  `transcribe_span_blocking` text byte-stable — flipping timestamp params
  changes whisper.cpp chunking and violates this).
- Splice functions are pure; the gate REPLAYS the production path and must
  never diverge from it (today: two inline decode loops mirroring
  `StreamDecoder` — a standing three-way lockstep hazard).
- Diagnostics are flag-only by default; enforcement graduates per class via
  explicit ear ruling (thread-C protocol).
- PII bright line: no verbatim meeting text in repo-committed fixtures —
  synthetic census fixtures only; the live gate stays the real-model parity
  pin.

## Goals / Non-Goals

Goals:
- Render fidelity becomes mechanically asserted: multiplicity/badge-aware
  pins, boundary-leak scan, words/sec ceiling, parent-link integrity.
- Class E fiction disappears from renders via the rate floor + duration
  floor, with the real words (already in the mixture rows) preserved.
- Fast-fail tier: census-replay harness catches splice/decode-contract
  defects in seconds; cold gate runs only after the replay tier is green.
- Ear economics: dictations land as fixture data; clips 07–12 drop out;
  future rounds are class-stratified with agent pre-triage.

Non-Goals:
- No automatic veto/stand-down beyond the class-E floors — deficit flags,
  leak flags, and boundary flags stay diagnostic until ear-ruled otherwise.
- No intra-row shape change from token walls in this change (task 4.4's
  phase B stays gated on wall-reliability evidence).
- No new GPU passes beyond the standard cold gate(s); no separation-model
  changes (bleed itself is a SEPARATION-layer concern, out of scope).

## Decisions

**D1 — Two-axis enforcement split.** Decode-content defects (hallucination,
word loss, bleed) are handled at the SYNTHESIS ACCEPTANCE layer (the rate
floor and duration floor degrade to the mixture render — the words already
live there); splice-geometry defects (boundary fragments, chronology,
edge leftovers) are handled at the SCAN + PIN layer (flag + gate pin; shape
changes only after ear-ruled evidence). Alternative considered: trim leak
fragments at splice time — rejected: trim direction is ambiguous without
token onsets (which copy is the leak?) and a wrong trim deletes the only
copy when the owning stream dropped the word (class-C interplay).

**D2 — Rate floor placement: two checkpoints, by cost.** The DURATION
floor is a cheap pre-separation filter on the span list (before
`port.separate` — a floor checked after separation wastes the GPU work it
exists to save). The words/sec ceiling is necessarily post-decode (it needs
the text) and lives in the synthesis acceptance (`vetted_stream_pair`
path). Both degrade to the byte-identical mixture render; the
words-per-second denominator is the stream's own span duration everywhere
(specs, census, gate — one definition), and — clip-09 ear ruling
2026-10-07 — where the stream split into utterances, the same ceiling
applies per UTTERANCE against that utterance's own walls: the rendered
granularity. (The phantom "You don't?" row was 2 words in a 0.2s utterance
— 10 w/s, invisible to the span-level check. Two alternative dials keyed
on window geometry or cross-stream text overlap were built and reverted
the same day: each broke an attested render — geometry blocked the S18
repair window, the n-gram bar fired on ordinary phrase echo ("would have
been"). Dial lesson recorded in the runbook: derive dials from the
render's physics signature, never from geometry or text-overlap
heuristics.) The mined evidence: all six
impossible spans' real words live in the mixture rows they replace, so the
degrade loses no attested text; the S16 showcase's synth rows are pure
fiction (its needles are carried by the plain row). Constants (initial values, all dials): `MIN_SYNTH_DURATION_SECS = 1.5`
(all six fiction spans are 0.37–1.03s), `MAX_STREAM_WORDS_PER_SEC = 8`
(~2.5× above measured human bursts), and `REPAIR_CHAIN_CAP_SECS = 35` (the
clip-03 edge chain needs ~33.3s). All recorded in the census with the
firing floor named.

**D3 — Scans are shared pure functions.** Boundary-token, words/sec, and
parent-link integrity scans live in `run_assembly` as pure fns over the
render rows; the production census writer and the gate call the same fns.
Rationale: a gate-private scan makes production blind (the ear protocol
needs census evidence); a production-private scan makes the gate assert
something production never computed. The boundary scan is provenance-
scoped to the synth row's TOUCHING edges only (tail vs the next row's
head; head vs the previous row's tail; a shared-edge gap guard mirrors
the duplicate scan's) and drops the 80%-of-shorter bar for those
pairings — the bar stays for the general duplicate scan.

**D4 — Decode-closure generalization FIRST.** `decode_span_synthesis`
gains a decode-closure parameter; the gate's two inline loops collapse onto
it. This is the prerequisite for every later synthesis change — without it
each change is a three-way lockstep edit across production + two gate
mirrors. The gate passes a closure mirroring `StreamDecoder::decode`
(engine + audit) so parity is structural, not copy-paste.

**D5 — Token census, phase A only.** A NEW adapter method
(`transcribe_span_blocking_tokens`) reusing the existing strict-segments
timestamp machinery — `transcribe_span_blocking` stays byte-stable
(structurally pinned: it sets `no_timestamps(true)`; the sibling flips that
locally, which BY DESIGN changes whisper.cpp chunking — so the token
decode's text is EXPECTED to differ and no equality test is written; the
census records the token decode's text sha256 alongside the production
decode's as a divergence NOTE, never an equality assertion). The production
method's byte-stability is pinned structurally (params test), not with a
model-backed unit test. Walls land as optional data on `StreamUtterance`
(ONE wall source; energy walls remain the fallback until ear-validated).
Shape change (phase B) is out of this change's scope until the
wall-reliability ear round passes.

**D6 — Census-replay harness with a two-tier fixture split.** The PII
bright line covers meeting-derived SYNTHETIC data too, so: (a) repo-
committed fixtures are PURELY synthetic — invented walls and word counts,
engineered defect shapes that no real span inspired; they prove the harness
mechanics and run the property tests; (b) meeting-SHAPED fixtures (the six
mined spans' walls/word counts, recorded-shaped texts) live in
`MEETILY_LOCAL_EVIDENCE_DIR`, loaded when present, skipped with a loud note
when absent — the cold gate remains their parity pin. The fake-decoder
contract is keyed by (span, stream) via per-call closure scoping: the
harness invokes the synthesis entry with ONE input per call and a closure
bound to that input's expected contract plus a call counter for stream
order — never a re-implemented loop. Rejected alternatives: real-meeting
fixtures in the repo (bright line), synthetic two-voice AUDIO (mocks the
separation under test; real separated streams are cached in the evidence
home for the span-probe tier), model-hash pinning (meaningless without real
decodes). Property tests: word conservation, wall monotonicity, provenance
containment, absorption fixpoint.

**D7 — Edge absorption: whole-row only, separate cap.** Repair chains may
extend to absorb a straddling edge row WHOLE (window grows to the row's
walls) under a new `REPAIR_CHAIN_CAP_SECS` constant, distinct from
`OVERLAP_MAX_SPAN_SECS` (the mass-span guard stays untouched — sharing the
constant would silently weaken mass-span eligibility). Absorb-whole-or-stop:
partial coverage duplicates absorbed words into the streams (documented
trap). Sequenced after D3's scan lands, so the re-run's leak status is
visible.

**D8 — Gate assertion upgrade set.** Needles: multiplicity + badge aware,
enforced from the fixture pin schema (S16/S18 hand-coded blocks migrate onto
it). New gate assertions: boundary-leak findings on pinned windows,
words/sec ceiling, parent-link integrity (synth_parent either an
atom link or a designed split-piece link; synth-span walls overlap parent
walls — the ..ddfdee misalignment class), and the same-donor exemption
narrowed to NOT suppress the three new scans.

## Risks / Trade-offs

- [Rate floor false-positives on genuinely fast speech] → the ceiling (8 w/s)
  sits ~2.5× above measured human bursts; the census records near-misses so
  the dial is retunable from evidence; degrade is to the mixture render,
  which is text-complete.
- [Duration floor drops a real sub-second per-voice render] → the mined
  evidence says the real words live in the mixture rows; the S16 needle
  pins move to the plain row they already occupy; the floor is a dial.
- [Boundary-token scan false-positive flood (short backchannels)] →
  calibrated on the census corpus BEFORE gating (P4's protocol); starts as a
  census flag + gate pin on attested windows only.
- [Warm-replay staleness after an engine/model bump] → the harness pins
  decode CONTRACTS, not model outputs; the live gate remains the parity pin
  and the ladder mandates a cold run per landed change.
- [Token-census decode diverges from the production decode] → sha256
  cross-check recorded per span; divergence is a loud census warning, never
  a silent wall source.
- [Vacuous-exemption narrowing changes the green board] → the fracture and
  duplicate scans' same-donor exemptions stay for attested pairs; only the
  NEW scans pierce them, so the baseline board does not shift under M1.

## Migration Plan

1. M1 lands as pure additions + acceptance-floor changes; the render for
   attested windows shifts only by the class-E fiction disappearing (the
   six sub-second spans degrade to their mixture rows — text-complete).
2. The gate's S16/S18 assertion blocks migrate onto the pin schema in the
   same change; the fixture gains the pin entries before the code flip
   (RED: pins fail against the current render where fiction exists).
3. Cold gate re-run after M1 (the ladder: replay green first).
4. M2 token census (census-only; no render change), M3 edge absorption
   (re-check leak flags after), M4 only on ear rulings.
Rollback: each milestone is an independent commit set; the floors and scans
are dials — reverting the constants restores the prior render.

## Open Questions

- Does the words/sec ceiling need per-language calibration (ES/CA vs EN
  rates)? Initial bar is language-agnostic; census near-misses will say.
- RESOLVED (propose panel): fixtures split per D6 — repo-committed purely
  synthetic; meeting-shaped in the evidence home.
- RESOLVED (propose panel): the post-M1 S16 pin shape — the window renders
  single-badged mixture rows; the migrated pin asserts mixture-only rows
  with the needles carried by the plain row at attested multiplicity, and
  the KNOWN-LIMITATION wording is updated BEFORE the cold gate (task order
  encodes this).
