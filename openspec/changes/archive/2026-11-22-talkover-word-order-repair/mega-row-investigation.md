# Mega-row investigation — talkover-word-order-repair (2026-11-18, user ruling)

Question: why does the S16 region render as one 32s row [1042.40,1074.42],
and can finer rows be restored so a row-aligned attested window fits under
the caps? Token-only findings (walls, counts, dates — no content).

## Findings

- **F-A — the mega-row is old, not a re-run artifact.** The render row
  [1042.40,1074.42] (badge Carlos Ruiz, 84 tokens, token_timestamps NULL)
  was CREATED 2026-09-14 — it is the render's stable shape, not
  something the 2026-11-18 re-run formed. The probe-record's "the
  re-render formed the mega-row" wording is corrected by this note.
- **F-B — what the 2026-11-18 re-run actually did: wiped the 2026-11-16
  repair.** The Nov 16 validated geometry (row-aligned walls
  [1024.61,1057.30], per-voice rows) existed only AFTER that run's
  phrase-loop repair replaced the region. A full re-run re-renders from
  `transcript_sources`, restoring source-derived rows — the repair-era
  rows were replaced, the mega-row returned, and the user's word-order
  complaint re-appeared the same day. The D4 content-hash gate catches
  exactly this (the entry binds the Nov 16 render's hash → stale today,
  loudly). Attestation entries are per-render-generation by design;
  every full re-run invalidates them.
- **F-C — the sources are fine; the render layer is the blocker.**
  `transcript_sources` holds three chunks in [1010.07,1083.46]:
  [1010.07,1032.31], [1032.31,1057.44], [1057.44,1083.46] — each ~22–26s,
  ALL with token timestamps. The render row [1042.40,1074.42] spans TWO
  source chunks (boundary 1057.44 — 0.14s from the attested window's end
  wall) and matches NO source boundary: the alignment+consolidation
  layer (diarization-segment walls, same-badge merge ≤ 3s gaps) built it,
  and the render row carries no token timestamps.
- **F-D — the unblock path is the deferred token-walls capability.** With
  per-word walls in the RENDER (the source layer already has them; they
  are dropped by the render pipeline), a straddling row could split at
  TOKEN boundaries — no proportional text guessing, no resurface trap —
  and the attested route could rebuild just the talk-over region inside
  the mega-row. That is the deferred `token walls phase B` work (fork
  issue #4, ear-round-gated, from the diarization-render-fidelity
  change). Until it lands, no admissible window (row-aligned, ≤ 35s
  chain cap) can both cover the S16 talk-over and separate it: the only
  row-aligned candidate IS the mega-row, and the separator collapses on
  that composition (probe run 2: stream0 4 tokens vs 44, both needles
  absent).

## Consequence

- S16 stays mixture (waiver already covers it) and its attestation entry
  awaits the token-walls capability — re-attestation per render
  generation is the interim cost of every full re-run.
- §3 wiring is unaffected: the route is measured end-to-end where walls
  align AND separation is strong (clean shape 7/7), and every failing
  shape failed closed in measurement.
