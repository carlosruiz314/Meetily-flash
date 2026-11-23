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

## Build findings (design amended accordingly)

1. **Convergence, not idempotence.** The carved span is fixed while the
   trimmed text shrinks, so the surviving tail token's derived interval
   inflates westward (last token owns the piece end) and a RE-INVOCATION
   may judge it against walls its original evaluation never saw. Deepening
   would re-judge a token whose original evidence was clean — the
   deletion-side risk the applied-science reviewer barred — so the rule
   stays single-pass conservative; a second invocation converges to zero
   trims (pinned). Production is single-invocation, pinned by the
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
reference meeting — 1376.52s, test ok. **Log capture was truncated to the
last 50 lines** (a `tail` pipe on the runner): the attested shapes'
records survived, the validated shape's census lines did not — a
re-scoped re-run (`MEETIFY_PROBE_SHAPES=validated`) covers that gap if
needed before enactment; the full-shape corpus run is the enactment-gate
artifact either way.

Captured results (token-only):

- `attested-east` (window [1024.61,1057.76]): `GUARD-CENSUS … would_trim=0
  point_rule=0` on BOTH legs — today's rebuilt streams carry NO fiction
  signature on the certified window under either candidate rule.
- `attested-west` (window [1024.61,1056.90]): identical — would_trim=0,
  point_rule=0, both legs.
- **No certified-content-region events** — the D8 bar's clause (iii) holds
  trivially on the certified shapes.
- `GUARD-PLACEMENT`: zero lines — the persisted rows these shapes read
  carry NO wall payload in the current DB state, so the placement-error
  census (field a0) has no corpus until the render-token-walls surface
  persists walls. Recorded as a limitation beside field f.
- `BASELINE-PIN pin1 want=1 got=2 FAIL` on both attested shapes — the
  probe's baseline drift check reads TODAY'S PERSISTED MIXTURE render,
  which legitimately carries the duplicated echo the user's S16 close-out
  documented; the amended pin (once) scopes the ATTESTED render, not the
  mixture baseline. Probe-harness scope note, predating this change; not
  a guard finding.

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
