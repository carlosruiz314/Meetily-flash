# Census review note — talkover-word-order-repair task 1.2

**Date:** 2026-11-18 (census measured 2026-11-18; production semantics, reference meeting)
**Record sha256:** `ebcd986c958382b2066e84495dabdd7e4671f8cfc5e7ad06b166e9a2b87b9f2e`
**Probe:** `tests/overlap_census_probe.rs` (extended per task 1.1; token-only; no Whisper decodes — separation + embeddings only)

## Counts (token-only)

- Candidates (trigger-fired, mass ≥ 0.3, ≥ 0.4s — production `OVERLAP_TRIGGER_MASS = 0.3`; the old probe header's "0.5" was stale): **132**
- Stood down at `MIN_SYNTH_DURATION_SECS` (1.5s) wall floor: **130**
- Passing the duration floor: **2** — `[1449.33-1451.00]` (1.67s, mass_max 0.975) and `[2537.00-2538.52]` (1.52s, mass_max 0.848)
- Stood down at `SYNTH_STREAM_MIN_RMS_RATIO`: **0** — see finding F1
- Span-level both-decisive + distinct (probe references; upper bound — production uses atom-level votes at validated walls, stricter): **57**
- Attested moments both present in the candidate set and BOTH stood down at the duration floor: `[192.92-193.76]` (0.84s) and `[1056.90-1057.30]` (0.40s, S16)
- Span durations: 128 of 132 candidates are ≤ ~1.0s (most 0.1–0.5s); the four longest are 1.42s, 1.52s, 1.67s, 1.67s

## Answers to the detector-precision question

The detector flags 132 spans per meeting, but this is NOT "132 talk-overs": the
overwhelming majority (128/132) are sub-second micro-overlaps (backchannels,
short crosstalk blips) that the 1.5s duration floor already stands down
harmlessly — mixture rows keep the correct text. Only 2 candidates per meeting
reach the synthesis stage at all. The flattening the user hears at attested
moments is not "flagged then degraded": those spans (0.40s, 0.84s) never reach
separation — the duration floor suppresses exactly the attested talk-overs.
That is a user-ruled dial (ear round 2, 2026-10-06) doing what it was set to
do; the repair change's attested-window seed route (design D4, amended
2026-11-18) is the mechanism that recovers those spans without moving the dial.

## Findings

- **F1 — the RMS eligibility floor is dead at the export-gain scale.** The
  production pre-normalization ratio (raw stream RMS / mixture-clip RMS,
  adapter semantics) reads ~1e5–1e6 for voiced streams in this environment —
  the ONNX export's output gain. The 0.1 bar therefore never fires (0 stood
  down; even a residue stream at ~1e2–1e3× clip RMS would pass). No dial moves
  without the user's ruling; recorded as a measured fact for the floor
  conversation. The within-clip energy split and the vote margins are
  unaffected (scale-free reads).
- **F2 — seed-visibility gap (design amended before any wiring).** As drafted,
  seeds required reaching separation, which sub-floor spans never do; both
  attested seed spans are sub-floor. Design D4 gained the attested-window seed
  route (a trigger-fired span inside an allowlist entry rebuilds regardless of
  the duration floor, hash-checked), so the promotion can actually fire at the
  attested moments. Decode-tripped seeds remain the second route (~2 candidates
  per meeting reach the stage where decodes could trip).

## Review outcome

User review pending (STOP gate): no promotion wiring before this review. The
offline decode-reliability stage (§2) proceeds — it touches no render path.
