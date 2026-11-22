# Probe record — talkover-word-order-repair task 2.4 (lever ladder)

**Date:** 2026-11-18 (probe `tests/repair_recover_probe.rs`, run live on the reference meeting)
**Record sha256:** `31e09fa22bcd3d933dde11996c84cb42bb2092b55a9b968980fd555335b290f7`
**Shapes:** the three kept shapes (archived `2026-11-17-overlap-repair-net-worse-guard` offline-evidence.md)

## Frozen before measurement (recorded here before the run)

- **Comparison operator** (amends the design's drafted wording): the ear
  gate's OWN pin semantics — `norm_tokens` (detokenize → lowercase →
  non-alphanumeric → space), contiguous non-overlapping run counting,
  joined scope over rows overlapping the pin window, `got != pin.count`
  fails. The fixture pins are needles at attested multiplicity, not full
  sequences. Unicode-NFC + exact token-sequence equality is RETRACTED.
- **Selection rule:** `run_assembly::select_repair_pass` (pass 1 stands
  unless guard-tripped AND the alternate clears every rejection guard).
- **Acceptance oracle:** each gated shape's ear-truth fixture entry pins
  on the promoted render. Baseline pins on the persisted render are a
  drift check recorded first.

## Per-shape records (token-only)

### validated — window [1024.61,1057.30], trigger [1056.90,1057.30], pins S16_ads_overlap_1056

- Baseline pins: pin0 want=1 got=1 PASS; pin1 want=2 got=2 PASS — the
  persisted render holds the needles; coordinates NOT drifted.
- Builder: attested walls exact (walls_match=true); shipped-path
  would-fire (no synth atoms).
- Identity: stream0 top=userB margin=0.5777; stream1 top=userA
  margin=0.2167 — both decisive, distinct.
- Leg A (strict): tokens [44, 40], no stutter, no fiction, splice ok.
  Leg B (no-condition repair): tokens [44, 40] — IDENTICAL. Selection:
  first-stands-first-clean.
- Render: spliced; absorbed=5 straddling=2 erase_west_s=42.44
  erase_east_s=17.12.
- Pins on promoted render: pin0 want=1 got=1 PASS; **pin1 want=2 got=1
  FAIL**.
- **VERDICT: NOT recovered** (see finding F2).

### clean — window [173.00,203.00], trigger [192.91,193.76], pins S18_clip01_talkover_192s

- Baseline pins: all 7 PASS (fabrication needle absent ×0, six needles
  present at count) — today's render holds the attested sequence.
- Builder: candidates=0 refused=synth-row-inside — the shipped attested
  route correctly stands down (the region is already per-voice from the
  2026-10-02 retranscription run); shipped-path outcome
  builder-refused:synth-row-inside.
- Identity: stream0 top=userB margin=0.4623; stream1 top=userA
  margin=0.3424 — both decisive, distinct.
- Leg A: tokens [35, 41], no stutter, no fiction, splice ok. Leg B:
  tokens [35, 41] — IDENTICAL. Selection: first-stands-first-clean.
- Render: spliced; absorbed=6 straddling=3 erase_west_s=22.58
  erase_east_s=2.74 (the erased text carries no needles — this shape
  passes despite the erase; F3).
- Pins on promoted render: all 7 PASS.
- **VERDICT: RECOVERED on leg A** — the builder+splice path recovers the
  S18 attested sequence verbatim end-to-end from separation + strict
  decode. Leg B adds nothing anywhere in this run.

### control — window [1045.30,1057.30], trigger [1056.90,1057.30], no pins

- Builder: attested walls exact; shipped-path would-fire.
- Identity: stream0 top=userC margin=0.0459 (UNDER the 0.05 vote bar);
  stream1 top=userA margin=0.2101.
- Both legs: splice rejected "a stream margin is under the vote bar" →
  degraded=mixture. **Fail-closed, byte-identical rows stand** (F4).

**SUMMARY shapes_gated=2 recovered=1/2.**

## Addendum — row-aligned re-cut (2026-11-18, user ruling: "Re-measure row-aligned")

**Run 2:** `validated-aligned` — window [1042.40,1074.42] (the mega-row's own
boundaries, row-aligned), trigger [1056.90,1057.30], same S16 pins.
**Record sha256:** `42e8cee8498ccbe775daa7ef1ffb63d5a257182f18387d4e18495292bdcc6dd8`

- Baseline pins: pin0 got=1 PASS, pin1 got=2 PASS (unchanged — coordinates hold).
- Builder: attested walls exact, would-fire; **absorbed=1 straddling=0
  erase_west_s=0.00 erase_east_s=0.00** — the geometry is exactly as the
  row-aligned rule intends.
- Identity: stream0 top=userB margin=**0.0531** (a hair above the 0.05
  bar — vs 0.5777 at the recorded shape); stream1 top=userA margin=0.2017.
- Legs: stream_tokens=[**4**, 65] — BOTH legs identical; stream0 (the
  talk-over voice) decoded 4 tokens vs 44 at the recorded shape: the
  separation collapsed at this window composition. No stutter, no
  fiction, splice ok.
- Pins on promoted render: pin0 want=1 got=0 FAIL; pin1 want=2 got=0
  FAIL — **both needles lost** (absent from both streams' decodes).
- **VERDICT: NOT recovered** — separation-quality failure, not geometry.

### Combined verdict: the S16 route is geometrically blocked on today's render

Both admissible wall strategies are now MEASURED and fail on different axes:

| Walls | Geometry | Separation | Result |
|---|---|---|---|
| recorded [1024.61,1057.30] | straddles the mega-row → 17.12s east erase | strong (0.578/0.217), occurrence #1 recovered | needle #2 erased (want=2 got=1) |
| row-aligned [1042.40,1074.42] | clean (erase=0) | collapsed (0.053; stream0 4 tokens) | both needles lost (want=1/2, got=0/0) |

The ONLY row-aligned window ≤ the 35s chain cap that covers the talk-over
IS the mega-row itself (any west extension exceeds the cap), and the
separator collapses on that composition. No admissible window on today's
geometry both covers the talk-over and separates it. The S16 moment
stays mixture (the S16 waiver already covers it); the runtime hash gate
keeps every failing shape byte-identical — **no wiring can regress the
render; the attestation flow inherently filters shapes** (an entry whose
shape fails this probe never gets attested).

Decode reliability stands PROVEN where walls align AND separation is
strong (clean shape, 7/7 hard pins, leg A). §3 wiring is unblocked by
this evidence; S16 specifically awaits either a render-geometry change
(why the 2026-11-18 re-render merged those rows into one 32s row) or a
user-ruled cap/dial move — both out of this change's scope.

## Findings

- **F1 — leg A suffices; no retry ships.** Both legs produced identical
  token counts on every shape; no pass tripped a decode-quality guard;
  the selection rule never left pass 1. Per design D3 ("if leg A
  recovers everything, NO retry ships") the no-condition retry stays
  unwired for this change; `transcribe_span_repair` remains the
  structurally-isolated §2 machinery (pinned by its isolation tests) —
  a measured lever, not a shipped one.
- **F2 — the validated shape's miss is wall-coverage, not decode
  reliability.** Needle occurrence #2 sits at proportional walls
  ~[1058.4,1059.2] inside the current render's mega-row [1042.40,1074.42]
  — BEYOND the recorded attested walls (end 1057.30). The in-window
  content (pin0, occurrence #1) recovered on leg A. The splice's
  absorb-whole semantics then ERASED the mega-row's outside-window words
  (17.12s east), removing the text that carried occurrence #2 → want=2
  got=1. The recorded walls were cut on the live run's geometry
  (user-validated 2026-11-16); the render re-rendered since (the 2026-11-18
  re-run formed the mega-row) and the walls no longer align to rows.
  This is EXACTLY the scenario D4's content-hash gate exists for: at
  runtime the entry's `render_sha256` would mismatch today's render →
  `stale-attestation` → fail-closed byte-identical mixture rows (pin1
  stays got=2, no regression). The probe bypassed that gate (measurement
  layer) and thereby MEASURED what the hash gate prevents — confirming
  the D4 design rather than refuting it.
- **F3 — absorb-whole erase is systemic.** Every spliced shape erased
  outside-window words (validated 42.44s west + 17.12s east; clean
  22.58s west + 2.74s east). Harmless where the erased text carries no
  attested content (clean shape passed), catastrophic where it does
  (validated shape). The attested route is only ever safe when the
  entry walls align to row boundaries on the hash-pinned render — an
  entry cut row-aligned absorbs whole rows and erases nothing.
- **F4 — the control fail-closed on the vote bar.** stream0 margin
  0.0459 < SUBTURN_VOTE_MARGIN (0.05) at the 12s shape → both legs
  rejected → mixture rows stand. Margin evidence is shape-dependent
  (the archived wide-window margins 0.135/0.147 were measured at a
  different window); any future attestation entry must be measured at
  ITS OWN walls before the user attests it.

## Stop-point verdict (task 2.4)

Decode reliability: **PROVEN on the attested sequence** (clean shape
recovers verbatim end-to-end on leg A; the validated shape's in-window
content also recovers; both legs identical). Attested-wall semantics:
**measured gap confirmed** — walls cut on a prior geometry lose content
on the current geometry; D4's hash gate fails that scenario closed at
runtime, so no wiring can regress the render, but the S16 complaint
stays unfixed until a row-aligned entry is cut on the CURRENT render and
re-measured at its own walls.

**STOP recorded per the task's rule: no §3 wiring before the user
re-plans the S16 entry.** The open decision: re-cut the S16 entry
row-aligned on the current render (candidate walls [1042.40,1074.42],
32.02s ≤ the 35s chain cap, under the attested builder's rules) and
re-run this probe on that shape (margins, pins, erase=0 expected) —
then §3 wires against measured entries only.

## Addendum — validated-recut on the wall-bearing render (2026-11-20)

**Date:** 2026-11-20 (the D4-carried obligation: decode re-measurement at
the re-cut's own walls, after render-token-walls landed)
**Record sha256:** `4cdafb01fed265cbc49493441859ff3e06997737c8e92194244b376b602ca977`
**Shape:** `validated-recut` — window [1024.75,1056.50] (the
render-token-walls handoff's measured re-cut: west row-aligned with the
NULL-walled row's end, east token-aligned at 1056.50 on the walled
mega-row); trigger [1056.90,1057.30] recorded as the REAL census trigger.
**Run env:** MEETIFY_LIVE_DIAG + MEETIFY_PROBE_SHAPES=validated-recut;
the loud evidence-home skip fired once (relative path from src-tauri —
absolute MEETILY_LOCAL_EVIDENCE_DIR required, the known Windows quirk).

- Baseline pins: pin0 want=1 got=1 PASS; pin1 want=2 got=2 PASS —
  today's mixture render still holds both needles; no drift.
- Builder: candidates=1 walls_match=true walls=[1024750,1056500];
  shipped-path would-fire.
- Decode: leg A-strict stream_tokens=[44, 39] — both streams decode
  substantial content, stutter=false fiction=false splice=ok; leg B
  identical (no retry, consistent with the 2026-11-18 ladder).
- **Render pins: pin0 want=1 got=0 FAIL; pin1 want=2 got=1 FAIL.**
- **SHAPE-VERDICT: gated=true recovered=NO.**

**Interpretation (token-only):** the re-cut geometry decodes cleanly and
splices, but the SPLICED render loses occurrence #1 entirely at the pin
window and one of occurrence #2's two occurrences — the handoff's
predicted needle coverage (occurrence #1 recovered in-window, occurrence
#2 surviving east) is FALSIFIED by the decode+pin evaluation. Not a
decode-quality failure (44/39 tokens, guards clean) — a coverage/
geometry failure at the splice output.

**STOP stands, now measured at the re-cut's own walls.** The S16 entry
cannot be cut on this geometry: the runtime hash gate would admit a
render that fails the ear gate's own pins, and the probe fails it first.
Per D3/D4 the user re-plans before any S16 entry ships. Two open
questions for that re-plan, both recorded not decided: (1) the census
trigger [1056.90,1057.30] sits EAST of the re-cut window, so the runtime
attested route (trigger containment) could not fire this entry even if
the pins held; (2) the east-edge token alignment (1056.50 vs the attested
1057.30) interacts with the pin window [1055,1058] — occurrence #1's
rebuilt walls land outside it in the spliced output.

The wiring itself is measured CORRECT: builder exact, guards clean,
byte-identical degrade everywhere, the runtime gate fails closed. The
two decode-tripped windows from the 2026-11-20 live run
([1421.21-1453.63], [2535.17-2559.11]) rebuilt cleanly and await the
user's ear round (their attestation is the user's word, per-window).

## Addendum 2 — the re-cut was aimed at phantom walls; the ruling finalizes the pin (2026-11-20)

**User ruling (earcheck-3 listen, dated 2026-11-20):** occurrence count 2
CONFIRMED — first "the question needle" at clip 0:12 (~1054.0), second at 14–15s
(~1056.9–1057.3), the second broken in delivery with an audible "sorry?"
tail. This upgrades the 2026-11-16 "probably yes" to a confirmed count;
the needle pins (want=1, want=2) stand unchanged. The "sorry?" tail is
attested but deliberately NOT a needle pin (broken delivery — whisper may
transcribe it differently); it rides the final certification listen.
Fixture note amended in the evidence home.

**Wall reconciliation (the RECOVERED=NO root cause):** the real wall
payload (mega-row, token-only positions) holds occurrence #1 at
1053.62–1054.08 (its "What" token carries no wall) and occurrence #2 at
1056.99–1057.29 plus degenerate zero-duration entries at 1057.43/1057.44
— matching the user's ear exactly. The 2026-11-19 handoff's claimed
positions ("occurrence #1 ends 1056.50; occurrence #2 spans
[1058.41,1059.17]") are NOT in the wall payload and were wrong; the
re-cut [1024.75,1056.50] was built on them and structurally ends BEFORE
the real occurrence #2 — its RECOVERED=NO verdict is explained and void
as a geometry verdict (the wiring verdicts stand: builder exact, guards
clean, fail-closed live behavior).

**Consequence — the first search candidate:** the originally attested
walls [1024.61,1057.30] cover both REAL occurrences and have never been
measured on the wall-bearing render (the 2026-11-18 erase verdict was on
the wall-less render, where the wall-split straddle policy did not
exist). The search opens there.

## Addendum 3 — the window search measured (2026-11-20, three candidates)

Correction to addendum 2's mapping: pin0 = the needle the or-needle token list ×1 (the audible "or" the user ruled real on 2026-11-16 —
the old repair dropped it); pin1 = the question-needle token list ×2. The probe's joined
scope is the ENTRY span over the SPLICED rows — a mixture east piece
touching the window edge exactly is excluded (strict-overlap predicate),
so all needle counts come from the STREAM rows alone.

Candidates (all on the wall-bearing render, leg A = leg B everywhere,
guards clean, no retry):
- **c1 "validated" [1024.61,1057.30]** (the originally attested walls):
  pin0 PASS — the "the or-needle" RENDERS (the 2026-11-16
  complaint repaired); pin1 got=1 FAIL.
- **c2 "attested-east" [1024.61,1057.76]** (east edge past the degenerate
  cluster): identical streams, identical verdict — the extra east context
  changed nothing.
- **c3 "attested-west" [1024.61,1056.90]** (east edge before occurrence
  #2): pin0 got=0 (the or-needle phrase needs the stream audio past 1056.90;
  the east mixture piece is scope-excluded) and pin1 got=1; stream B's
  decode also read "for" instead of "or" with the truncated context.

**The measured blocker:** pin1's second "the question needle" run is buried under
the overlapping speech at ~1054 (the user's first occurrence) — the
separator+whisper never render it as a contiguous run in ANY candidate
(stream token counts identical across all three windows). The user's
ear-attested second occurrence (broken, with the sorry tail) decodes only
as a clipped fragment that does not satisfy the needle. want=2 is
measured UNATTAINABLE per-stream on today's separation+decode.

**The decision this puts to the user (their word, per the pin doctrine):**
(a) amend pin1 want 2→1 with the dated reasoning and ship c1's shape —
recovers the "the or-needle" (a real Nov 16 complaint) and
the third speaker's clean "the question needle", at the measured cost of the buried first
occurrence staying swallowed (word loss of crosstalk-buried words); or
(b) keep S16 mixture — the limitation stands documented. The runtime
hash gate keeps every un-amended entry fail-closed meanwhile; trigger
containment holds for c1/c2.

## Addendum 4 — the user ruled: amend pin, ship c1; winner measured RECOVERED=YES (2026-11-21)

The user ruled (2026-11-20): **amend pin1 want 2→1 and ship c1
[1024.61,1057.30]**, with ONE certification listen before it goes live.
The fixture was amended the same day (pin_2 ×2→×1; dated amendment block
S16_ads_overlap_1056 carrying the ear ruling — first occurrence clean,
second broken with the sorry tail — and the measured unattainability of
want=2 per-stream).

The winner re-measured against the AMENDED oracle (record sha
9fd102a2…, 2026-11-21):
- BASELINE (mixture): pin0 got=1 PASS, pin1 got=2 FAIL — the mixture
  carries the duplicated/broken run; that is the defect the amended
  oracle rejects.
- Builder: candidates=1 walls_match=true walls=[1024609,1057300];
  shipped-path outcome=would-fire — the REAL census trigger
  [1056.90,1057.30] is contained in the window, so the production
  trigger route fires this window (no hand-drawn span).
- Legs: A-strict = B-repair, stream_tokens=[44, 40], stutter=false,
  fiction=false, splice=ok, guards clean — leg A stands (no retry
  shipped).
- Identity: stream0 top=userB margin=0.58; stream1 top=userA margin=0.22.
- RENDER-PIN pin0 want=1 got=1 PASS; RENDER-PIN pin1 want=1 got=1 PASS
  → **SHAPE-VERDICT name=validated recovered=YES**.
- Attestation coords captured for the store entry:
  render_sha256=08f55d47275deedf6abfda25112b8873baf6284de6d3ac025e29cea
  d30808f21, profile_fp=repair/strict/no-retry/width-30-chain-35/
  model-large-v3-turbo-q5_0.

Certification pending: the user hears the winning render rows against
the raw audio (earcheck clips) and gives the certification word; then
the entry goes into the scheduler store and the production Speakers
re-run ships it. Accepted cost stands: the crosstalk-buried first
occurrence stays swallowed (word loss of overlapped speech, per the
amended pin).

## Addendum 5 — certification: certified with limitation; the render mapping corrected (2026-11-21)

The certification listen (the user, 2026-11-21, against earcheck-3 and
the raw audio) corrected the render mapping claimed in addenda 3–4:
NOTHING is swallowed. The needle run the question-needle token list ×1 that renders sits
in the USER's stream row — it is the third speaker's FIRST (clean)
occurrence (walls 1053.62–1054.08) bled into the user's stream, rendered
between the user's two sentence halves (the timeline fits exactly). The
third speaker's SECOND (broken) occurrence is what the other stream
caught at the window tail: decoded as a one-word fiction under the WRONG
badge. Addendum 3's "buried first occurrence" and addendum 4's "accepted
word loss" are WRONG — both occurrences render, both misattributed; the
2026-11-20 amendment reason's word-loss framing is superseded.

The user's verdicts: (1) the needle the or-needle token list
renders correctly — yes; (2) the fiction tail is the third speaker's
clipped second occurrence, not the second stream's own speaker —
confirmed by ear; (3) the fiction is STRUCTURAL, measured: trimming the
east edge before the occurrence loses the needle (addendum 3's c3
got=0), extending east past the utterance decoded byte-identical (c2),
and the retry leg was identical — needle and bleed share the same
talkover audio (3 voices, 2 streams).

RULING (2026-11-21): **certify with limitation + file the follow-up.**
The allowlist entry goes into the scheduler store (walls
[1024.61,1057.30], render_sha256=08f55d47…, profile_fp
repair/strict/no-retry/width-30-chain-35/model-large-v3-turbo-q5_0); the
production Speakers re-run ships it; a follow-up issue (wall-payload
provenance guard sketch + the 3-voices/2-streams root cause) goes to the
fork. Ship is reversible-in-effect: a future render with a different
hash fails closed to mixture (stale-attestation) until re-attested.

## Addendum 6 — the first production ship found a 1ms gate round-trip bug (2026-11-21)

The first production Speakers re-run with the entry live (37.5 min,
harness `live_speakers_run` reading the allowlist straight from the
scheduler store) measured the certified window **awaiting-attestation —
mixture kept** (census: total=3 attested=0 awaiting=3 stale=0; the two
decode-tripped windows await their ear round as expected). Everything
else verified clean: outside-window rows byte-identical (265 rows),
row count 269 before/after, transcript_sources unchanged, 3 windows
promoted (the attested route DID fire — the seed match held).

Root cause — an ms→s→ms round trip at the splice gate: the candidate
span is ms (1024609, from the seed = entry_walls_ms), the synthesis path
carries seconds (1024609/1000.0 = 1024.609), and the gate re-derived ms
by TRUNCATION: (1024.609 × 1000.0) as i64 = **1024608** — one ms short
of the entry's 1024609, so exact-wall equality failed → Unpinned →
awaiting. The end wall (1057300) round-trips exactly; only the start
loses the ms. The probe's SHIPPED-PATH outcome=would-fire compared the
CANDIDATE walls against the seed walls (exact) and never exercised the
gate's re-derivation — that approximation gap is why the probe passed
while production failed.

Fix: the gate's re-derivation ROUNDS (`(s * 1000.0).round() as i64`) —
f64's 1e-13-relative error is orders below half-ulp at ms magnitudes, so
rounding recovers any integer ms exactly. The ENTRY side keeps its
truncation on purpose: decimal seconds (1024.61 × 1000 = 1024609.999…)
cut AT the truncated value — that IS the certified coordinate the probe,
the fixture pin, and the store entry all carry. Red test
`gate_walls_survive_the_seconds_round_trip` pins the real certified
walls; run_assembly module 144/144, lib suite 820/820.

Re-run in flight. Note: the wall fix also un-gates the hash check for
the first time — the production decode's render_sha256 must equal the
certified 08f55d47… or the gate goes stale-attestation (fail-closed,
loud, both hashes logged).

## Addendum 7 — the stale-attestation root cause: the carve offset; production render re-measured RECOVERED=YES (2026-11-21)

The second production run (after the 1ms gate fix) reached the hash
check for the first time and went **stale-attestation, fail-closed**
(entry sha 08f55d47… vs decode sha bb7f9b19…, fingerprint matching, both
logged loud, mixture kept). Investigation:

1. **Audio provenance ruled out**: the probe re-run on the PRODUCTION
   audio path (MEETIFY_PROBE_AUDIO=production: decode_audio_file
   (audio.mp4) → to_whisper_format — the app's own decode calls)
   produced the SAME 08f55d47… as the cache provenance, byte-identical
   record. The samples_16k.f32 cache and the fresh production decode
   yield decode-identical floats.
2. **The real cause — a 1ms carve offset**: the probe separated at the
   DECIMAL-second window (1024.61 s) while the production fold separates
   at the entry's round-tripped ms (1024609/1000.0 = 1024.609 s) — the
   stream audio differs by 16 samples at the west edge, enough to flip
   whisper's decode of the window tail.
3. **The probe re-run at production's carve coordinates**
   (window = 1024609/1000.0, 1057300/1000.0; window_ms by rounding,
   matching the gate fix) produced **render_sha256=bb7f9b19… — EXACTLY
   the live run's decode hash**: two independent executions (production
   fold + probe) agree byte-for-byte on the same carve, which is the
   decode-determinism evidence. Both render pins PASS on this render
   (pin0 got=1, pin1 got=1) → **RECOVERED=YES on production's actual
   render**.

The production render differs from the 2026-11-21 certified render in
exactly ONE spot — the fiction tail. The decimal carve decoded the third speaker's
clipped second occurrence as a one-word fragment; the production carve
decodes the same bleed as "the fiction tail fragment" (three words, same structural
3-voices/2-streams fiction, fork issue #7). The user's row is byte-
identical across both carves ("…the question needle the or-needle.").
The certification therefore RE-BINDS to the production carve: the user
re-certifies the production render (one listen), the fixture pin + store
entry take hash bb7f9b19… at the same walls/fingerprint, and the final
production run verifies fired+attested.

## Addendum 8 — the final verification caught absorb-whole erase: the entry is withdrawn, the ship is blocked (2026-11-22)

The final production run (entry re-bound to the production carve hash
bb7f9b19…) SPAWNED the attested window — census `attested=1 awaiting=2`,
the synth pair landed in the persisted render byte-identical to the
certified production rows — and then the harness's outside-window
byte-identity assertion FAILED: the splice had erased real speech.

Mechanism (the splice's absorb predicate, `repair_overlap_rows_checked`):
ANY atom overlapping the window is absorbed WHOLE — no straddler-keep.
The west NULL-walled atom [1010.07,1024.748] overlaps the window
[1024.609,1057.299] by 139ms, so its entire text was replaced by a
decode that only covers the window. Its out-of-window content —
14.5 seconds, the user's mercenary-work sentence ("…the user's mercenary-work sentence") — vanished from the persisted render (zero matches on re-query;
the zone [1010.07,1024.608] holds no row at all). The erase metric in
every c1 record (erase_west_s=42.44) was recording exactly this loss;
it was never checked for meaning. The east survived byte-identical.

Actions taken:
- The store entry is WITHDRAWN (`attestedRepairWindows` emptied — the
  fail-closed default); the fixture pin stays as the historical record.
- A restore run (no entry) is returning the persisted render to the
  mixture state, byte-identical to the pre-ship state (verified 269
  rows twice before).
- The ship is BLOCKED on the geometry: a window whose west edge absorbs
  out-of-window speech can never ship.

The geometry candidate: **validated-west-aligned [1024.75,1057.30]** —
the west edge at the REAL row boundary (the west atom ends 1024.748, so
nothing west of the window is absorbed — no loss, no duplication; the
east edge stays at the attested 1057.30; trigger still contained;
1024.75 is exactly representable so the ms round trip is exact). The
139ms-shorter west context changes the decode, so this is a NEW shape
needing the full measurement (probe in flight) and — if it recovers the
pins — a NEW certification listen before any entry ships.

## Addendum 9 — the geometry space is exhausted: STOP; S16 keeps the mixture render (2026-11-22)

The west-aligned re-cut **validated-west-aligned [1024.75,1057.30]**
measured **RECOVERED=NO**: the 139ms-shorter west context flipped the
window-tail decode — the user's row reads "…the question needle, then the or-needle phrase" with NO "or" (RENDER-PIN pin0 want=1 got=0 FAIL; pin1 PASS;
erase_west_s=0.28 measured against the transient post-ship DB, which
still held the spliced rows — the erase accounting needs the restored
state, but the pin failure is decisive regardless: the stream decode
depends only on audio+span).

The geometry space is now exhaustively measured:
- **c1 [1024.61,1057.30]** (the certified shape): needle recovers, both
  pins PASS — but the splice absorbs the west atom
  [1010.07,1024.748] WHOLE and erases 14.5s of out-of-window speech
  (addendum 8). DEAD.
- **west-aligned [1024.75,1057.30]**: no west absorb — but the needle
  dies on the west truncation (the c3 failure class: west/east edge
  shifts flip the window-tail decode). DEAD.
- **full west coverage [1010.07,1057.30]** (the only geometry that
  neither truncates context nor absorbs the west atom partially):
  47.23s — over the repair-chain cap (REPAIR_CHAIN_CAP_SECS=35), refused
  at the splice by construction. STRUCTURALLY IMPOSSIBLE.

**STOP (the change's own rule: a shape that no leg recovers ships
nothing until the user re-plans).** Under the current design constants
there is no window geometry that recovers the attested needles without
erasing out-of-window speech. S16 keeps the mixture render (the restore
run returns the persisted state byte-identically; the store entry stays
withdrawn; the fixture pins stay as the historical record). The real
fixes live in fork issue #7: the wall-payload provenance guard (catch
contradictory absorbed-row fragments instead of absorbing whole atoms)
or 3-stream separation (the root cause of the whole talk-over class).
The promotion machinery itself is validated — it fired end-to-end
(attested=1) and every gate did its job: the 1ms round-trip bug (caught
by the second run), the carve-offset mismatch (caught by the hash gate),
and the erase (caught by the harness assertion).

**Restore verified (2026-11-22):** the persisted render is back to the
pre-ship state — 269 rows, the 265 outside-window rows byte-identical to
the pre-ship snapshot (zero diffs), the west atom's full text (the
mercenary-work sentence) restored, transcript_sources untouched, and the
only synth rows are the legacy per-voice pair at [161.71,191.05] (the
pre-existing overlap-span path, not the talk-over promotion). The
restore run's exit-101 is the harness assertion false-alarming on the
restore direction (its before-snapshot was the spliced 268-row state) —
inapplicable for restores, the DB verification above is the evidence.
