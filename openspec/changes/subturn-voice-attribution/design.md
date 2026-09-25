# Design: subturn-voice-attribution

## Failure chain (measured, not assumed)

1. pyannote segmentation holds one label across a fast voice handoff — no
   frame-mass valley → no `label_change_candidates` / trust-zone slot change
   / valley-flip candidate exists inside the merged turn.
2. AHC labels the whole turn with one cluster (often the voice that dominates
   the turn, sometimes outright wrong — the DISAGREE class).
3. Render honors turns (D9), so the whole exchange persists under one badge.
4. Census: 45 MIXED + 6 DISAGREE rows of 229.

## New engine pass (last, additive)

In `derive_turns_from_masses`, after the rescue splice produces final
`turns: Vec<TurnOut>` and final centroids `used`:

```
turns = subturn_voice_pass(turns, samples, extractor, &used);
```

Per turn:
- Skip turns shorter than `SUBTURN_MIN_TURN_SECS` (1.0s).
- `voiced_chunks(samples, start, end)` → energy-gated voiced sub-runs
  (20ms RMS hop, adaptive baseline+10dB gate — probe-proven), each run
  chunked to ≤ `VOICE_CHUNK_MAX_SECS` (3.0s), dropping runs below
  `VOICE_CHUNK_MIN_SECS` (0.3s).
- Embed each chunk; cosine vs every final cluster centroid; keep the vote
  only if top-1 beats top-2 by `SUBTURN_VOTE_MARGIN` (0.05).
- `subturn_segments(votes, turn, cluster)` splits on *qualifying runs*:
  a run of same-cluster consecutive votes qualifies when it has
  ≥ `SUBTURN_MIN_RUN_VOTES` (2) chunks and ≥ `SUBTURN_MIN_RUN_SECS` (1.2s)
  of voiced time, or a single chunk ≥ `SUBTURN_LONG_SINGLE_SECS` (2.0s).
  A qualifying run of cluster V ≠ current segment voice opens a new
  segment at the run's first chunk start. Segment voice = the qualifying
  run's cluster; the head before the first split keeps the turn's cluster
  (its audio is undecided or minority — safe to keep the AHC verdict).

Why sustained-run gating: TitaNet sub-second windows misvote (census: the
ear-attested UserB "Oh, man." [14.42–15.08] votes UserA). A lone
0.66s vote cannot split; a 2s+ single chunk or a 2-chunk 1.2s+ run can —
sub-second back-channels stay absorbed (documented limitation, unchanged),
while multi-sentence absorptions (the ear failure) split.

## TurnOut mapping

First segment inherits the turn's `continues_previous` and
`low_confidence`; subsequent segments get `continues_previous: false`
(a voice change is a new turn start for stamping) and inherit
`low_confidence`. `dur_secs` recomputed; `attached_secs` assigned to the
first segment only (attachment already happened at piece level).

## Downstream invariants

- Cluster space unchanged (no renumbering; registry matching unaffected).
- Rescue seams and rescue candidate semantics untouched (pass runs after).
- Alignment: finer `diarization_segs` feed boundary-anchored assignment —
  boundaries still snap to sentence gaps, so no mid-sentence badge change
  can be introduced (both-bars preserved by construction).
- Stamping/continuation facts consume the finer turns as ordinary turns.

## Hexagonal boundaries

- `run_assembly.rs`: pure `voiced_chunks` + `subturn_segments` (+ consts).
  `voiced_chunks` takes `&[f32]` samples and consts — no I/O.
- `run_engine.rs`: `subturn_voice_pass` — the only place the embedding
  extractor meets the vote logic; adapter already behind the port.

## Verification ladder

1. RED unit tests on `subturn_segments` (8 adversarial cases).
2. Ear gate (offline replay from provenance-checked caches — exercises the
   full engine incl. this pass): 16/16 pins, 0 fractures, "Oh, man." text
   anchor → Speaker 1.
3. Row-voice census re-run on the re-persisted DB: MIXED + DISAGREE must
   collapse (target: MIXED ≤ half, DISAGREE = 0) without breaking any ear
   pin.
4. Live re-run + full text dump for ear check.

## Residuals accepted

- Sub-second back-channels remain absorbed (pre-existing documented
  limitation; the pass deliberately does not split on short evidence).
- Chunks with no decided votes keep the surrounding segment's voice.
- A turn whose audio is entirely undecided keeps its AHC verdict.
