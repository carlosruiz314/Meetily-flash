# Closing the ear-truth gap to 14/14 (S4/S5/S6) — exploration

Date: 2026-09-06. Evidence: `closure-gap-probe-20260906.log` (same dir),
produced by `frontend/src-tauri/tests/closure_gap_probe.rs`:

```
MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan \
  --test closure_gap_probe -- --ignored --nocapture
```

## TL;DR

1. **14/14 as pinned is unreachable.** S6 (Cynthia's "Yeah" at 32.65) and S7
   (one voice 32.0–38.0) contradict each other, and three independent signals
   side with S6: the "Yeah" is Cynthia. Ceiling = **13/14** (S4✓ S5✓ S6✓
   S7✗). 14/14 requires the user to re-pin S7 with this evidence.
2. **The recorded root cause was wrong.** pyannote does NOT lack the signal —
   the pipeline discards it. Mid-window decodes are slot-consistent at
   confidence ≥0.99 across every covering window. The merged frame cache
   (edge-of-window overwrites at 1s seams), the <0.3s silence absorption in
   `speech_runs`, and the speech-to-speech requirement in
   `corroborate_split` jointly destroy both real handoffs.
3. The three failed scan attempts cascaded because they scanned the corrupted
   merged track. A fix that instead reconstructs trust-zone labels from the
   per-window tracks the engine **already receives** has a measured
   prediction: S4/S5/S6 pass, S7 fails, others unchanged (must be gate-verified).

## Measured facts (all three signals agree)

Anchors (verbatim clip answers): USER 29.50–32.16, CYND 39.00–39.93, RIC
2802.08–2820.13. pyannote votes use mid-window frames only ([w+2, w+8) of
each 10s window — the trust zone; 1s step, production geometry).

### The 30.0 handoff (S5) — real, silence-mediated

- pyannote: P3(0.99–1.00) at 27.5/28.5/29.5 → **silence 0.71–0.80 at 29.9** →
  P2(0.76–1.00) at 30.5/31.5. Same two slots across all 8 covering windows.
- TitaNet 1.5s windows: U-affinity 0.190 → 0.519 → 0.675 → 0.758 → 0.879
  across 29.0→30.0; C falls 0.298 → 0.02. FINE windows put the crossover at
  ≈29.4–29.6.
- F0: female 216–235 Hz through 29.25, unvoiced 29.5–30.0, male 119–154 Hz
  from 30.0.
- Piece halves: head 27.34–30.00 U=0.104 C=0.263 (Cynthia); tail 30.00–32.16
  **U=0.964** (pure user).

### The 32.65 "Yeah" (S6) — Cynthia, not the user

- pyannote: 32.7 = **P3** (0.83–0.90) in every covering window — the same
  slot as 27.5–29.5; flanked by silences at 32.3 (0.92) and 33.3 (0.96+).
- TitaNet: yeah span 32.60–33.00 C=0.356 vs U=0.077.
- F0 at 32.75–33.0: 229 Hz (female).

### The disputed 34.66–38.64 stretch — two halves, not one

- 34.66–36.21 = **Cynthia**: P3 at 35.0 (0.86–0.96); C=0.259 vs U=0.116; F0
  216–320 Hz.
- 36.25–38.64 = user: P2 at 37.5/38.3 (0.95–0.99); U=0.670; F0 147–184 Hz.
- The banked banter-probe value disp34=0.574 (user) is a duration-dominated
  BLEND of the two halves; it was never evidence that the whole stretch is
  the user.

### S4's fragment (25.31–26.43) — Cynthia

F0 232–250 Hz (female); affinity C=0.237 vs U=0.191. The S4 failure is the
same missing 30.0 split: Cynthia's 27.34–29.95 speech is welded into the
user-labeled turn.

## Revised root cause (supersedes the archived "no signal" finding)

The engine builds pieces from the merged frame cache, and:

1. merged cache frames near window seams are written by edge-of-window
   decodes (merged-track flips land on integer seconds — 30.00 exactly);
2. `speech_runs` absorbs silence gaps < 0.3s (`MIN_RUN_SECS`), so the 0.1–0.2s
   pause at 29.9 disappears and 27.34–32.16 stays ONE run;
3. `label_change_candidates` excludes silence-adjacent transitions;
4. `corroborate_split` requires a speech→speech label change in both adjacent
   windows — both real handoffs are P3→sil→P2, rejected by construction.

Meanwhile the per-window trust-zone decodes (already delivered to the engine
as `window_tracks` for `corroborate_split`) hold the whole story.

## Fixture pass-structure math

Gate semantics (`ear_truth_gate.rs`): single_voice = all turns overlapping
the span by >0.25s share one label; voice_change_at = exactly one
consecutive-turn label change in-span, within tolerance of the pin.

- S5 requires a change in [29.5, 30.5]. Measured: 29.95. ✓ reachable.
- S6 requires exactly one change in [31.9, 33.4]. Measured: ≈32.5 (start of
  Cynthia's "Yeah" run). ✓ reachable — but the "Yeah" turn (0.32s) overlaps
  S7's span [32.0, 38.0] by 0.32s > 0.25 → **S7 must fail**.
- 14/14 would require the S6 change ≤32.25 into a voice that then holds
  through 38.0 — contradicted by the measured Cynthia "Yeah" and the
  Cynthia 34.66–36.21 stretch. The user's clip 05/06 answers are right;
  clip 10's blanket "one voice 0:00–0:06" missed a soft 0.3s backchannel
  (and a 1.5s stretch).

## Fix design (if pursued)

Inside `derive_pieces`, reconstruct a **trust-zone label track** per speech
run from `window_tracks` (for each frame take the covering window whose
center is nearest — no production merge change, no new model calls), then:

- split candidates = mode-filtered slot changes A→B (B≠A) across an absorbed
  short silence, at the silence midpoint;
- corroboration = the same A→B event present in each covering window's
  trust zone (existing per-window event machinery, silence-mediated variant).

Prediction: S4 ✓ (Cynthia head detaches; fragment joins Cynthia's side),
S5 ✓, S6 ✓, S7 ✗. The change is global (input track), so the full gate must
re-verify the other 11 entries — the scan-cascade risk applies to the
extent that downstream boundaries shift; unlike the scans, the input is
strictly closer to ground truth (mid-window decodes are pyannote's most
confident).

## Correction to the archived design notes

The archived claim that enrollment would let "the mixed piece's halves and
the 32.65s blip verify against clean fingerprints" is overstated: without
the split there ARE no halves — the 27.34–32.16 blend embedding reads
U=0.808 (tail-dominated), so reference-anchored clustering labels the whole
piece user-side even with a perfect Cynthia reference. Enrollment alone
closes at most S4 (fragment side).

## Consequences to flag if the fix lands

- Whisper row 30.09–34.92 spans three speakers ("Yeah, sure, sure, sure…
  right? Where"); post-split, that row's text attaches by midpoint and will
  visibly mis-attribute. Fixture labels are unaffected; UI text is.
- Invariant holds: the "is Ricardo" row starts lowercase →
  `continues_previous=true` stamped.

## Options

A. Implement the trust-zone split; gate decides. Expected 13/14 (S7 flips to
   fail). User may then re-pin S7 to the measured structure → 14/14.
B. Sign off S4/S5/S6 (and S7 discrepancy) as KNOWN-LIMITATION. Status quo 11/14.
C. Enrollment first — max +1 (S4), does not touch S5/S6.

## Result (2026-09-06, option A implemented)

User chose A. Fix landed in `run_assembly.rs` (trust-zone label track,
silence-mediated slot-change candidates, ≥2-window trust-zone corroboration;
5 new unit tests; purely additive to the existing merged-candidate path).

Verification (final state): lib 605/605; synthetic gate 3/3; ear gate
`trustzone-gate-20260906.log`: **12/14, 0 invariant violations over 255
turns** (was 11/14 over 239).

- S4 ✓, S5 ✓, S6 ✓ — as predicted. The banter region now reads
  `25.31–29.92 sp1 | 29.92–32.16 sp0 | 32.65–36.08 sp1 | 36.08–38.64 sp0 |
  39.00–39.93 sp1` — matching the probe's measured structure everywhere.
- S7 ✗ — as predicted, but on the 36.08 boundary (the real Cynthia
  32.65–36.08 stretch), not the "Yeah" (the engine merged the "Yeah" and the
  34.66–36.08 stretch into one Cynthia turn across the pause).
- S14 ✗ — unexpected; adjudicated by `s14-boundary-probe-20260906.log`:
  **the engine's new boundary 161.36 is right and the fixture pin 162.78 is
  wrong.** pyannote flips slot at ≈161.4 (six windows, consistent within
  each), F0 steps 235→125 Hz at 161.4, TitaNet crossover completes by
  161.0, and the user's own coarse clip answer ("0:06" = 161.0) is 0.36s
  from the engine boundary vs 1.78s from the old pin. The 162.78 pin was
  measuring the crosstalk/pause after the real handoff.

State after the fix: 12/14 as pinned; 14/14 available via two
evidence-backed fixture re-pins (S14 → 161.36; S7 → measured structure),
both user decisions.

## Final state: 14/14 (2026-09-06, user-adjudicated)

The user adjudicated both remaining entries by ear on fresh clips
(check-clips-2026-09-06/, neutral names, extracted from the meeting audio):

- **clip_A (158.0–165.0)**: "voice changes at clip second 0:03" ≈ 161.0 →
  confirms the engine boundary 161.36, refutes the old 162.78 pin. S14
  re-pinned to 161.36 (fixture note documents the re-attestation).
- **clip_B (33.8–36.8)**: "2 voices" — Carlos "Gotcha" ≈33.8, **Cynthia
  "Where is Ricardo?" ≈33.8–34.8**, Carlos "I don't know" from ≈35.8. The
  old S7 single-voice pin dies on the user's own attestation; rewritten as
  multi_voice 33.8–36.8. This also corrects the long-standing assumption
  that "Where is Ricardo" was the user's line — it is Cynthia's, which is
  exactly where the engine attributes it (sp1 turn 32.65–36.08).
  Documented residual: Carlos's ≈0.5s "Gotcha" is absorbed into the
  Cynthia turn (sub-second backchannel; engine deliberately does not split
  sub-floor pieces).

Confirming gate run `trustzone-gate-20260906-final-14of14.log`: **14 passed,
0 FAILED of 14 entries; 0 invariant violations over 255 turns.** The
session objective — outputs match the user's answers — is met at the engine
layer, with every fixture pin either a verbatim user number or a
machine-refined position inside the user's attested structure (refinement
documented per entry; the ear attests structure everywhere, exact seconds
only where a clip answer gave one).

Follow-up candidates (not blocking): sub-second backchannel splitting
(the "Gotcha" class); Whisper rows spanning multiple speakers garble text
attribution (30.09–34.92 row); enrollment for real-name labels.

## Live render failure + adversarial review loop (2026-09-06, evening)

The first live Speakers re-run rendered shredded sentences (a three-way
split of the "five years" sentence, 128 "Unknown Speaker" slivers, 507 rows
vs 237 sources) — the fixture gate never looked at the align+persist layer
the user actually reads. The user's charge — overindexed on 14 proofs,
disregarded the rest — was correct. Root causes, all measured:
(1) a zero-width piece at 12.07 became a zero-second turn (fixed: derive_pieces
retain guard); (2) this meeting has no token timestamps, so the PROPORTIONAL
aligner chopped every row at every engine boundary and D9 skipped the
same-speaker row merge (fixed: gap-borrow by nearest turn EDGE on the engine
path + same-label fragment re-merge + D9 amended to run the row merge);
(3) the gate's frame-mass cache was stale (pre-trust-zone output) — the
engine on the app's real fresh input differed from everything validated
(cache deleted, regenerated, provenance-keyed; 14/14 re-validated on the
production decode).

Three adversarial reviewers (algorithms / persistence dataflow / verification
integrity) ran to two rounds; dispositions:
- ACCEPTED: render-layer gate inside the ear gate (persist invariants now
  fail the gate); turn-edge borrow with measured cap (inter-turn gaps p50
  0.86s / p95 2.84s / p99 5.91s, 163 gaps → 3.0s cap ≈ p95+); legacy borrow
  path restored byte-identical; D9 amendment; CJK space guard; sliver
  proximity guard in derive_pieces; cache provenance sidecar (model hash +
  geometry + format version); S7b companion entry pinning the attested
  Carlos resumption (36.08, user coarse 35.8); clip manifest
  (tests/fixtures/check_clips_2026-09-06.json) — audio stays out of git.
- REJECTED with evidence: "14/14 rests on deleted cache" — the current
  cache is exactly what the fresh run and the confirming run validated
  (reviewer mis-read local vs UTC mtimes).
- DOCUMENTED, not fixed: trust-zone blind zone at meeting edges (first/last
  ~2-3s cannot reach 2-window attestation; single-window attestation is the
  rejected seam-artifact class); sliver-turn gate asymmetry; gate reads the
  live DB reference pool (personal-machine gate); straddling-row stamp fall-
  back (NULL → text heuristic); remaining known defect — Whisper timestamp
  lag misattributes individual words at true speaker changes (the "Ricardo"
  class), a transcription-layer fix.

Render-gate evidence: `trustzone-gate-20260906-round2-15of15-render.log` —
222 turns, 15/15 entries, 0 invariant violations, RENDER: 237 rows in →
531 fragments aligned (108 Unknown → 0 after borrow, 0 within cap) → 422
merged rows; zero-dur 0, unmerged same-label pairs 0. The cache is
provenance-keyed (model hash + geometry + format version in
`gate_frame_masses.meta.json`); this run validated the current cache.

Gap-percentile measurement (cited for GAP_BORROW_MAX_MS = 3.0s): derived
from the fresh-input gate run's TURN dump — 163 inter-turn gaps, sorted;
p50 0.86s, p90 1.86s, p95 2.84s, p99 5.91s, max 20.77s.

## Adversarial review convergence (two rounds, five reviewers)

Round 1 (three reviewers: algorithms SHIP / persistence FIX-FIRST /
verification-integrity FIX-FIRST) → fixes applied. Round 2 (two reviewers:
fix-verification FIX-FIRST-narrow / fresh-eyes FIX-FIRST) → narrow fixes
applied. Dispositions beyond those already listed above:

- ACCEPTED: symmetric sliver guard (merged-label candidates + run-edge
  proximity, not just trust-zone candidates — 30–80ms phantom turns measured
  at sp0→sp1 handoffs); render-gate Unknown assertion scoped to the cap
  (far orphans stay Unknown by design); GAP_BORROW_MAX_MS + nearest_turn_span
  made pub and shared with the gate; D9 residual documented (consolidation
  deletes absorbed rows → an adjacent same-speaker turn pair loses its
  second first-row stamp → text-heuristic fallback computes the same fact);
  name-collision divergence documented (two clusters resolving to one
  enrolled name merge across a real cluster boundary in production rows but
  not in the raw-label gate — enrollment-quality risk, pre-existing);
  trust-zone blind zone at meeting edges documented in-code.
- REJECTED with evidence: "sliver guard one-sided" — accepted candidates are
  pushed into bounds inside the same loop, so tz-vs-tz proximity is enforced.
- SURFACED FOR USER ADJUDICATION → RESOLVED 2026-09-07: the fixture is
  identity-blind (structure-only). The engine attributed the "five years"
  sentence (9.38–13.03) to the Cynthia cluster while an early fixture note
  said "User voice" — the user adjudicated: **Cynthia's voice**, matching the
  engine and the acoustic probes. S1/S2 identity notes corrected (structure
  pins unchanged); the derived consequence — "Yeah. That's right. Oh, man"
  (13.42) is the USER's — follows from the attested 13.03 change and the
  probe affinities.
- Follow-ups recorded: enrollment for real names + identity checks; Whisper
  timestamp-lag word misattribution at true speaker changes; sub-second
  backchannel splitting (the "Gotcha" class).

## S2b: second user adjudication round (2026-09-07)

The user adjudicated two more identities while reviewing: "five years" is
CYNTHIA (S1 note fixed; also resolves the months-old S1/S2 identity
contradiction — the voice from 13.03 is the USER), and **"Oh, man"
(14.78–15.84) is CYNTHIA again** — a second change after 13.03. S2 narrowed
to [12.0,14.5]; new S2b entry (voice_change_at 14.78±1.0) encodes the
attestation. Gate `trustzone-gate-20260907-s2b-known-miss.log`: **15/16,
S2b failing by design** — the engine's only in-span change is 16.12s (the
post-silence resumption): pyannote decodes 14.78–16.12 as silence, the
"Oh, man" words exist only in Whisper's transcript, and the geometric gap
borrow currently pins them to the user's turn.

Fix path (next change, "gap-speech attribution"): when Whisper-heard text
sits inside a pyannote-silence gap, embed the gap audio and attribute it to
a meeting centroid by voice (not geometry) — that creates the missing
≈14.78 Cynthia boundary and passes S2b. Same class as the "Gotcha" residual
in S7.
