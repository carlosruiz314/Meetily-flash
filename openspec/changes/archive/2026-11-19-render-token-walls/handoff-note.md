# Handoff note — render-token-walls 3.4 (2026-11-19)

The paused `talkover-word-order-repair` change resumes §3 on this render.
Token-only facts (probe record sha256
`6bc84ab077227c802c53dcdb88e8f20249921092a78e69771539b44f3f254166`).

## What the render looks like now

- The rendering is wall-bearing: census rows=269 walled=194 null=75
  violations=0 (run #2, after the two fixes this change landed).
- The S16 mega-row [1042.40,1074.42] carries a real per-word wall list
  (4122-char JSON) — the east straddle is token-splittable.
- West of S16, rows [982.17,1024.75], [1024.75,1027.86],
  [1027.86,1032.31] are NULL-walled: source chunk [1010.07,1032.31]'s
  DTW token list (68 pieces vs 50 text words) fails the alignment
  validator's backtrack tolerance, so its fragments render wall-less and
  the fail-closed merge poisons every merged row they join.
- Baseline pins hold on the new render: S16 2/2, S18 7/7. Builder
  outcomes unchanged (validated would-fire; clean correctly refused,
  synth-row-inside). Separation margins at the recorded shape identical
  to the 2026-11-18 measurement (0.5777 / 0.2167).

## The concrete unblock (measured geometry)

The recorded attested walls [1024.61,1057.30] cut through two rows whose
west side is NULL-walled — but the walls enable a re-cut that needs NO
NULL-side split:

- Window [1024.75,1056.50] (~31.75s ≤ the 35s chain cap).
- WEST edge 1024.75: row-aligned with the NULL-walled row's end — that
  row is NOT absorbed (the overlap predicate excludes a touching row);
  its 42s of outside-window text cannot be erased.
- EAST edge 1056.50: the token boundary nearest the attested wall
  1057.30 on the now-walled mega-row (occurrence #1 ends 1056.50;
  occurrence #2 spans [1058.41,1059.17]). The mega-row splits there:
  its west piece is absorbed inside the window, its east piece keeps
  occurrence #2's tokens under their real walls.
- Needle coverage: occurrence #1 is inside the window (stream decode
  recovers it — measured 2026-11-18, margins decisive); occurrence #2
  survives untouched in the east piece. The S16 pin window's joined
  stream holds both needles at attested multiplicity.
- The two NULL-walled rows inside the window are absorbed whole and
  REPLACED by stream rows — no erase, no resurface; their being
  wall-less is irrelevant to the splice.

## Carried-over notes for the resuming change

- The attestation entry's `render_sha256` SHOULD cover the wall payload:
  walls are part of the render's identity and regeneration re-derives
  them (design D6 of this change).
- The S18 attested-route surface is unchanged (builder-refused,
  synth-row-inside) — the phrase-loop region stays per-voice.
- If a future entry needs a NULL-side split, the validator's backtrack
  tolerance on source chunk [1010.07,1032.31] is the blocker — a
  dial-adjacent question for the user, never a unilateral move.
