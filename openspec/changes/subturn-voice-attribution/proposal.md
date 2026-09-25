# Proposal: subturn-voice-attribution

## Why

Ear verdict 2026-09-22 on the persisted cde5c264 rendering: "These labels are
way off. Constantly absorbing my sentences into UserB's and vice versa."
The row-voice census (TitaNet sub-window votes on every persisted row;
`openspec/exploration/row-voice-census-20260922.log`) confirms it is
meeting-wide, not a render regression:

- 229 persisted rows: 172 agree with the voice evidence, **45 are MIXED**
  (two or more voices decided inside one row/badge), **6 are DISAGREE**
  (unanimous voice contradicts the badge for the whole row), 6 undecided.
- Worst cases: a 74.6s row badged UserC holding an entire
  UserC↔UserB exchange; a 31.5s row badged UserA holding UserB and
  UserC windows; "One more thing to throw a wrench in all your plans."
  badged UserA when every voice window says UserB.

Root cause: pyannote segmentation merges whole exchanges into single turns
where voices hand off without a frame-mass valley (fast handoffs, crosstalk).
The render honors engine turns by design (D9: "engine turns are final"), so
one merged turn renders every sentence under one badge — the absorption the
ear hears. The existing valley-gated flip scan cannot help: it only splits
where pyannote itself dips, and these turns have no dip.

## What

Add a sub-turn voice-attribution pass at the end of the engine assembly:
after turns, flips, and rescue are final, slice each turn into energy-gated
voiced chunks (≤3s), embed each chunk with TitaNet, and vote against the
final cluster centroids (margin-gated, same 0.05 bar the gap rescue trusts).
Split a turn ONLY where a *sustained* run of one voice accumulates
(≥2 chunks and ≥1.2s, or a single ≥2.0s chunk); each resulting segment
carries the qualifying run's cluster. Short contrary votes — including
TitaNet's known misvotes on sub-second back-channels (the ear-attested
UserB "Oh, man." window votes UserA) — are absorbed into the surrounding
voice, so every ear pin holds by construction.

## Boundaries (hexagonal)

- Pure split logic (`voiced_chunks`, `subturn_segments`) lives in
  `run_assembly.rs` — no I/O, fully unit-testable (adversarial RED tests).
- The pass wires into `run_engine::derive_turns_from_masses` after the
  rescue splice; it consumes only `samples`, the extractor port, and final
  centroids. Downstream (segments, rescue seams, alignment, stamping)
  consumes the finer turns unchanged.

## Cost

~1400–2000 extra TitaNet embeddings per live run (≈ +1–2 min on the
~6-minute run). Offline validation needs no new caches (frame-mass + samples
caches already provenance-pinned).

## Adversarial tests

RED tests on `subturn_segments`: lone contrary vote never splits;
sustained run splits + re-badges; unanimous-contrary turn rebadges with a
head sliver keeping the original cluster; alternating sub-second
back-channels never split; multi-change turn yields 3 segments; empty votes
keep the turn; abstain gaps don't break a run. Gate re-run must keep all 16
ear pins, 0 sentence fractures, and the "Oh, man." text anchor green.
