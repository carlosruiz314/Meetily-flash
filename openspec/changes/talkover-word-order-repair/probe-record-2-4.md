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
