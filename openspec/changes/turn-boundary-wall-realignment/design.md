# Design: turn-boundary-wall-realignment

## Mechanism (one pure function)

`realign_straddling_atoms(units: &mut [(Vec<UnitWord>, bool)], diarization: &[DiarizationSegment])`
in `alignment.rs`, called once after `build_logical_units`, before any
assignment path (boundary-anchored, per-atom majority, vote override). It sees
only spans and turn labels — no audio, no models, fully unit-testable.

For each adjacent turn pair (T1, T2) in meeting order:

1. Skip when `T1.speaker_id == T2.speaker_id` (no voice change) or
   `T2.sustained_split` (a split inside one speaker's speech — mid-sentence
   continuation is legal there).
2. Boundary `B = T2.start_ms`; require `|T1.end_ms − B| ≤ 250 ms` (a real
   seam, not a loose gap).
3. Collect all atoms (sentence_atom_ranges over every real-span unit), sorted
   by start; find the first straddler `A` (`A.start < B < A.end`).
4. Qualification (all must hold, else the boundary is left alone):
   - `A`'s last word ends a sentence (a complete sentence moves as a unit;
     no punct-position check is possible — the piece→word merger fuses the
     period into the word — nor needed: per the ear law the voice does not
     change mid-sentence, so a complete-sentence straddle of a genuine
     voice change is a wall error by construction);
   - side-past-boundary `A.end − B ≥ 300 ms` (a normal turn-final sentence
     has its period at the boundary — side ≈ 0 — and never qualifies);
   - head room `next_atom.start − B ≥ 300 ms` (the far turn has unclaimed
     voiced head — in the normal case the next atom starts immediately);
   - scale `room / A.dur ≥ 0.5` (no wild compression).
5. Re-anchor: linearly rescale `A`'s word walls into `[B, B + room]`.
   Monotonicity is automatic (`prev.end ≤ A.start < B = new start`); the
   existing output-seam non-overlap clip stays as the final guard.

Calibration is not tuned to look right — every constant is pinned by the S7c
measurements (side 429 ms, room 639 ms, boundary tolerance covering the exact
seam) and enforced from below by the negative tests.

## Hexagonal boundaries

- Domain-pure: lives in `alignment.rs` alongside the other wall/atom logic;
  no I/O, no models, no Tauri. `commands.rs` and `run_engine.rs` are
  untouched — the aligner already holds both inputs (units, engine turns).

## Interaction with voice votes

Wall-atom votes are computed on the ORIGINAL token walls (commands.rs). After
realignment, the S7c atom's span `[B, B+room]` has zero overlap with its old
smear vote chunk (old end 36069 < B 36080), so the stale UserB vote cannot
override; the atom falls through to turn containment → UserA. Verified
explicitly in the unit test (vote list included, mirroring production).

## Adversarial tests (RED before GREEN)

1. S7c reproduction (integration, in-crate): one row with the real fixture
   token walls, turns T1 sp1 [32650–36080] / T2 sp0 [36080–38640], production
   vote list → "I don't know." renders as its own row `[36080, 36719]` under
   Speaker 0; "Where is UserC?" stays Speaker 1; Gotcha still flips to
   Speaker 0 via its vote.
2. Normal turn-final sentence: period at the boundary (side < 300 ms) →
   unchanged.
3. sustained_split boundary → unchanged.
4. No head room (next atom starts within 300 ms) → unchanged.
5. Different-voice guard: same-speaker adjacent turns → unchanged.
6. Property: realignment never makes spans overlap or invert (covered by the
   output-seam clip + explicit monotonicity assertions in test 1).

## Gate

`S7c_idontknow_userA` pin window moves to the corrected location
(`audio_start_ms ∈ [35800, 36400]`), expects Speaker 0, waiver branch removed,
fixture `known_limitations` cleared (already done). The full 18-entry suite,
fracture scan, and duplicate scan must stay green in one run.
