# Probe record — wall-payload-provenance-guard (apply phase, 2026-11-23)

All text in this record is repo-side and token-only: counts, walls,
buckets. No meeting-derived words. The local evidence fixture is read at
runtime by the probe only.

## Red → green

- Red state (stub, pre-implementation): 13 of 19 guard tests FAILED,
  6 passed trivially (the keep/no-op cases a no-op stub satisfies).
  Red list: contradicting-tail trim, census-mode byte identity, needle
  protection, piece-drop walk, emptied-stream absent decode, CJK
  whole-token, mixed payloads, multi-token walk, hash binding, composed
  splice, census note grammar, pre-pass wiring scan, degenerate/abstained.
- Green state: 19/19 guard tests pass; full lib suite 839 passed / 0
  failed (twice — pre- and post-census-refactor); census_replay_harness
  4/4; repair_attestation_gate 6/6.
- Post review-round-1: 22 guard tests (the census-note-applied-only gate
  and the multi-cover precedence fixture added, plus a crate-wide
  wiring-scan test in the harness); lib suite 841 passed / 0 failed;
  census_replay_harness 5/5.

## Build findings (design amended accordingly)

1. **Convergence, not idempotence.** The carved span is fixed while the
   trimmed text shrinks, so the surviving tail token's derived interval
   inflates westward (last token owns the piece end) and a RE-INVOCATION
   may judge it against walls its original evaluation never saw. Deepening
   would re-judge a token whose original evidence was clean — the
   deletion-side risk the applied-science reviewer barred — so the rule
   stays single-pass conservative; re-invocation converges within a
   bounded cascade (the fixture pins the third invocation at zero — the
   second may deepen once; review-round-1 wording correction). Production is single-invocation, pinned by the
   source-scan test (exactly one call site, between ordering and
   admissibility). tasks 1.5 amended from `guard(guard(s))==guard(s)`.
2. **Vetting precedes the guard.** A Some-text piece with ~0 duration
   fails the pair's utterance-rate gate before the guard runs — the only
   production degenerate shape is None text. The guard's own degenerate
   skips are defense-in-depth.
3. **Source-scan self-count trap.** The wiring-scan test counts its own
   `include_str!` body; the needle literal is split at the test site (the
   established `concat!` idiom), and the guard's wiring comment avoids the
   gate-marker literal.
4. **The census trace sink was structurally dead (review round 1).** The
   push gate read the Trim-only `report.trims` counter, so in the only
   posture that passes a sink (Census) no trace could ever fire: the
   probe's `point_rule=0` readings were vacuous, the D8 breakout fields
   unreachable, and the bare-point verdict additionally taken only on
   trimmed tokens (one-sided against the compared rule). Fixed: mode-aware
   delta counter, bare-point verdict over EVERY visited token (trims plus
   the terminal keep), keep-side divergence (`point_rule > trims`) pushes
   a trace of its own; corpus re-run post-fix below. A reviewer-caught
   fixture bug in the new precedence test (a straddling word fully
   covering the second token) was fixed by shifting the word east of the
   half-open boundary.

## Census (design D8) — what landed and what it logs

- Guard census fields live in the shared walk (one implementation — the
  census cannot diverge from the shipped rule): per window per stream,
  `GUARD-CENSUS` lines log would-trim counts under the shipped
  interval-majority rule AND under the original bare-point rule on the
  same visited tokens (the rule-selection comparison), the decisive
  coverage fraction, the decisive covering-word width, and the
  terminal-punctuation breakout.
- `GUARD-PLACEMENT` lines log the placement-error distribution
  (proportional char-fraction wall vs real token wall) per persisted
  payload row — the empirical bound for the last-token population.
- Census mode at merge: `commands.rs` runs the pre-pass log-only for
  every route before admissibility; renders byte-identical; evidence
  rides the seed census (`tail-trimmed:<n>` beside the outcome token) and
  the `REPAIR-GUARD` line.
- **Limitation recorded:** the own-stream real-wall-delta field (D8 field
  f) needs the stream decode's per-token walls, which the decode seam does
  not surface (`String` only) — blocked on the render-token-walls surface
  (issue #830 / the paused wall-persistence change). The census runs
  without it; its acceptance bar is evaluated on the remaining fields
  until that surface lands.
- **Corpus:** the live census run on the reference meeting is scoped to
  the certified-window shapes (validated / attested-east / attested-west)
  as the first leg; the second-meeting leg waits on the user naming a
  meeting. Trim enactment on any route is unlocked only by the D8 bar
  (would-trims tail-only, predominantly single-token, zero events inside
  certified-content regions, placement error below the covering-word
  scale) on BOTH legs with the user's dated review.

## First live census leg (2026-11-23, production audio provenance)

Run: `MEETIFY_LIVE_DIAG=1 MEETIFY_PROBE_AUDIO=production
MEETIFY_PROBE_SHAPES=validated,attested-east,attested-west` over the
reference meeting — 1376.52s, test ok. The first capture was truncated
to the last 50 lines (a `tail` pipe on the runner); the re-scoped
re-run below closed that gap.

**Review-round-1 correction:** the captures above carry honest
`would_trim` counts, but their `point_rule=0` readings were VACUOUS, not
measured — the census trace sink was structurally dead (build finding 4):
no trace could fire, the probe's point-rule sum always read zero, and
`GUARD-CENSUS-TRACE` silence was indistinguishable from a dead path. The
corpus was re-run post-fix; that run's numbers supersede the pre-fix
point_rule readings.

**Post-fix census run (2026-11-23, measured):** all three certified
shapes over production audio, 1595.49s, exit 0, full log to a temp file:

- `GUARD-CENSUS shape=validated leg=A-strict would_trim=0 point_rule=0`
- `GUARD-CENSUS shape=validated leg=B-repair would_trim=0 point_rule=0`
- `GUARD-CENSUS shape=attested-east leg=A-strict would_trim=0 point_rule=0`
- `GUARD-CENSUS shape=attested-east leg=B-repair would_trim=0 point_rule=0`
- `GUARD-CENSUS shape=attested-west leg=A-strict would_trim=0 point_rule=0`
- `GUARD-CENSUS shape=attested-west leg=B-repair would_trim=0 point_rule=0`

This time the zeros are MEASURED: the counters are live, and a trim or a
keep-side bare-point fire would emit a `GUARD-CENSUS-TRACE` line (none
appeared). Today's rebuilt streams carry NO fiction signature on the
certified windows under either candidate rule — the D8 clause (iii)
(zero events inside certified-content regions) holds on all three
shapes, now on a working sink. Other lines, unchanged in meaning:

- `GUARD-PLACEMENT`: zero lines — the persisted rows these shapes read
  carry NO wall payload in the current DB state, so the placement-error
  census (field a0) has no corpus until the render-token-walls surface
  persists walls. Recorded as a limitation beside field f.
- `BASELINE-PIN pin1 want=1 got=2 FAIL` on every shape — the probe's
  baseline drift check reads TODAY'S PERSISTED MIXTURE render, which
  legitimately carries the duplicated echo the user's S16 close-out
  documented; the amended pin (once) scopes the ATTESTED render, not the
  mixture baseline. Probe-harness scope note, predating this change; not
  a guard finding.
- `SHAPE-VERDICT attested-west recovered=NO` observed — the probe's own
  recovery measurement for that shape, consistent with the S16
  close-out's keep-mixture ruling for the west window (probe-harness
  scope; the guard is census-mode and changes no bytes).

## Determinism / toolchain note

The frozen operator folds NFD/NFC via the `unicode-normalization` crate
(a guard-side mismatch deletes a real word, so ES/CA accents must never
read as different words — the ear gate's matcher lacks the fold; the guard
is the stricter superset). A future toolchain upgrade that flips a rare
codepoint's fold would move hashes — which fails closed to
stale-attestation (the safe direction).

## Certification story (unchanged)

No store entry is written and no certification listen happens in this
change: the machinery merges render-inert (census mode everywhere). A
future attestation round re-pins the post-guard hash per the spec
scenario "Pinned hash binds the post-guard render" and re-certifies by
ear. Trim enactment is the user-gated follow-up flip after the census
review.
