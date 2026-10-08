# Runbook — diarization-render-fidelity

## The verification ladder (design D6)

Every synthesis/splice change climbs this ladder IN ORDER. Never spend a
cold gate run on a shape the replay tier has not reproduced.

1. **Unit** (`cargo test --lib`, milliseconds): the pure gates and scans —
   floors (`impossible_rate_stream_decode_degrades`,
   `duration_floor_stands_down_before_separation`), the splice degrade
   family (`*_degrades`), scans (`boundary_leak_*`,
   `parent_link_*`), entry contract (`decode_span_synthesis_*`).
2. **Warm replay** (`cargo test --test census_replay_harness`, seconds):
   synthetic token-only fixtures drive the REAL splice
   (`decode_span_synthesis` + fake decoder + `apply_overlap_synthesis` +
   `apply_loop_repairs`) and the property tests (piece-word containment,
   wall monotonicity, provenance containment, absorption fixpoint).
   Decode-contract changes (re-slices, per-chunk decode, byte drift) fail
   HERE, not at a ~70 min gate.
3. **Span probe** (env-gated live, one span): `utterance_wall_probe.rs` /
   `span_decode_probe.rs` reproduce the target span's separated decode on
   real audio before any engine-facing change ships.
4. **Cold gate** (`tools/run_ear_gate.bat` of the owning change, ~70 min):
   the real-model parity pin. Requires the replay tier green FIRST (task
   5.5 encodes this gate-side; the ladder encodes it process-side).

## Pre-triage protocol (task 8.1)

Before handing clips to the user, the agent predicts the defect class per
clip from the census profile alone (duty cycle, chunk count, RMS margins,
words/sec rates — all token-only in `gate-runs/` logs). Clip sets are
class-stratified; the prediction is recorded BEFORE the user listens so
the ear ruling tests the prediction, not the reverse.

Prediction record (2026-10-07, clip-04 as the worked example, task 5.4):
census profile says the region's mass span [2537.00-2538.52] (1.52s) sits
just above the duration floor but at 22.4 w/s — the rate floor rejects it
at acceptance; with no synthesized rows there is no stutter seed, and the
mixed row carries no 5-word consecutive repeat, so no loop seed either →
PREDICTION: the [2535.17-2559.11] repair window no longer builds and the
mixture row renders. The fixture pins (`clip04_mixed_row_render_2535`,
hard) encode the prediction; the cold gate adjudicates it before any ear
time is spent.

## Calibration records

- 2026-10-07 (task 3.4): `fidelity_census_probe.rs` over the persisted
  render — synth-side boundary shared-chunks max at 1; plain-side one
  chunk=4 plain/plain pair (duplicate-scan territory). Dial
  `BOUNDARY_LEAK_MIN_CHUNK` stays 3. Parent links: 28 links, 0 findings
  (post ..ddfdee re-point).
- 2026-10-07 (clip-09 ear ruling, RUN 1 → RUN 3): the wide repair window
  [1421.21-1453.63] swallowed the class-E stood-down span [1447.30-1447.96]
  and its per-voice render carried a PHANTOM cross-stream duplicate
  ("You don't?" — stream 0 re-decoding stream 1's own words) plus an
  unheard tail. User ruled the mixture render preferred (fixture
  amendment, user_confirmed 2026-10-07). DIAL HISTORY (three iterations,
  one day): (1) GEOMETRY — windows covering floor stand-downs never fire —
  RUN 2 falsified it (also blocked the S18 window [161.71-191.05], which
  covers its own floor span yet renders the attested-good state);
  (2) CROSS-STREAM 3-GRAM — RUN 3 + the s18_leak_probe falsified it (the
  two real S18 speakers share "would have been"; n-gram bars over
  whole-window texts always over-fire); (3) LANDED — the class-E
  words/sec ceiling extended to UTTERANCE walls in `vetted_stream_pair`:
  the phantom is 2 words in a 0.2s utterance (10 w/s), fiction at the
  granularity actually rendered. LESSON for future dial work: rulings
  choose between RENDERS; derive the dial from the render's PHYSICS
  signature (rates against real walls), never from window geometry or
  text-overlap heuristics — both over-fire on ordinary language.

## Token-wall evidence plan (task 6.3, phase A → phase B gate)

The gate's repair replay emits `CENSUS-TOKENWALL` lines per repair stream
(walls, segment count, token-decode sha + production-decode sha as a
divergence NOTE — the texts differ BY DESIGN). The cold gate run of task
5.5 records these for every long repair window. PHASE B (consuming the
walls as `StreamUtterance` shape — the class-B chronology fix) is gated on
an ear round validating wall reliability on exactly that recording: the
user replays clips against the token-wall boundaries; only walls the ear
confirms graduate into the splice. No shape change before that ruling.

## Open measured items

- Census-vs-live yield mismatch (P5 data gap): the probe counts the
  persisted render; the gate counts the replay. Compared only at a cold
  gate run — 5.5 records the delta.
