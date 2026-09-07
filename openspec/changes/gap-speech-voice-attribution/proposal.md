## Why

Real speech that pyannote decodes as silence is structurally invisible to the run-assembly engine: `derive_pieces` only creates pieces inside `speech_runs`, so there is nothing to embed, cluster, or turn. In cde5c264 the engine holds `[hole 14.78–16.12]` between turns `13.42–14.78 sp0` and `16.12–20.30 sp1`, and the row `14.78–15.84 "Oh, man"` — Cynthia's, per the user's ear — overlaps no turn and renders under Carlos's badge (production's fine-row borrow). The ear-adjudicated region (clips C/D/E, 2026-09-07, verbatim in `check_clips_2026-09-06.json`):

- Change Cynthia→Carlos at **≈12.0** ("Yeah") — the engine misses it (boundary at 13.42, 1.42s off). Sub-run change miss → S2 is the gate's expected-fail KNOWN-LIMITATION; fix is a separate follow-up change.
- Cynthia's "Oh, man" starts **≈15.8** — the engine's 16.12 boundary is within tolerance, so **S2b passes today**; the defect is the rendered text under the wrong badge.
- Legacy row timestamps here are skewed in both directions (ear-attested ≈1.4s late / ≈1.0s early), so any fix must place boundaries at the **voice onset**, not at row starts.

Retuning the speech gate would ripple globally; the boundary signal is gone upstream (`sil ≥ 0.93`-class suppression). The only truthful source is voice evidence: embed the gap audio, compare against the meeting's final centroids, place the splice where the voice starts.

Panel history: 3 review rounds converged the mechanism (10 reviewers total); the ear re-pins (clips D/E) then revised placement and fixture structure, absorbed in this version.

## What Changes

- **Gap-rescue pass in the engine (post-clustering)**: after centroids finalize, every silence gap that (a) overlaps transcript text by ≥ the promotion floor on the RAW span∩gap (0.8s; union of intersecting rows clipped to the gap; one `embed_slice`-convention embedding per gap), (b) separates two DISTINCT turns (adjacency-based; interior and meeting-edge gaps abstain), (c) yields a decided voice identity (margin ≥ the ambiguity margin) contradicting the modeled borrow winner (midpoint of span∩gap, ties → earlier turn, within GAP_BORROW_MAX_MS), and (d) has a DETECTED VOICED ONSET inside the gap, splices a synthetic promoted sub-floor piece from **the voice onset** to the coalescing gap edge before a second `resolve_turns` pass. Trim-or-abstain: no confident onset → no splice (an untrimmed row-start splice is forbidden — it would place boundaries at skewed timestamps and regress S2b, which passes today). Snapshot semantics; existing coalescing/flag rules natively; single-cluster meetings abstain; everything else falls through to today's borrow.
- **Doctrine amendment (scoped, via MODIFIED blocks)**: transcript text spans gain a third sanctioned use — locating text-bearing silence gaps (with an explicit labeling-unit exception); text content still never sources boundaries or labels. The run-assembly requirement gains the rescue layer plus a bounded exception to the runs shed-to-cap. The ear-truth gate requirement's stale enumeration is replaced by the fixture file as source of truth, with auditable user-confirmed amendment records.
- **Fixture re-pins (user-confirmed 2026-09-07, clips C/D/E)**: S1 → `[9.38, 11.8]` single_voice; S2 → `change_at 12.0±0.75` — expected-fail KNOWN-LIMITATION (the engine's missed "Yeah" change; separate follow-up class); S2b → `change_at 15.8±0.75` — passes TODAY, must still pass after. An `amendments` map (`user_confirmed` + `reason`) plus a non-live fixture-lint test make every waiver auditable.
- No change to the pyannote pass, clustering, piece derivation, resolve_turns semantics, the render/borrow/merge layer, or the fallback path.

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: one ADDED requirement — speech in pyannote-silence gaps is attributed by voice (gap-rescue layer with voice-onset placement); three MODIFIED requirements — the queue-phase requirement (transcript-row doctrine gains the third use; labeling-unit exception), the run-assembly requirement (rescue layer + bound exception + the zero-overlap attach case extended to abstaining silence gaps), and the ear-truth-gate requirement (fixture file as source of truth; auditable waivers; evidence location pointed at `openspec/exploration/`; the `single_voice` overlap threshold stated as the >0.25s the gate code implements).

## Impact

- `frontend/src-tauri/src/audio/speaker/run_assembly.rs` — pure `rescue_candidates` decision function (ordered slices, no hash maps; onset passed in by the engine) + unit tests
- `frontend/src-tauri/src/audio/speaker/run_engine.rs` — rescue wiring: gap/span derivation, voice-onset detection (signal-level), embedding I/O, pre-rescue turns, splice, second resolve pass, debug-zip fix
- `frontend/src-tauri/tests/ear_truth_gate.rs` — amendments machinery + fixture lint; render-line evidence
- `frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json` — S1/S2/S2b re-pins, S2 KNOWN-LIMITATION, `amendments` map
- `frontend/src-tauri/tests/fixtures/check_clips_2026-09-06.json` — clips C/D/E verbatim answers (done)
- `frontend/src-tauri/tests/closure_gap_probe.rs` (or sibling ignored probe) — de-risk measurement before engine code
- Task 0 lands the uncommitted base + re-pins as ONE commit (nothing pushed without explicit sign-off)

## De-risk gate (before engine code)

An ignored probe must answer, on the real meeting, using ENGINE-REAL geometry: (1) current turn set + pyannote votes for 12–17s and 32–38s on the committed record + measured borrow distances at the target gap; (2) **voiced-onset detection rehearsal** inside 14.78–16.12 — does energy-based detection find ≈15.8 (the mechanism depends on it); (3) identity votes for the raw window [14.78, 15.84] AND the voiced sub-window, from BOTH transcript sources with the production reference set (margin, best, silence-gate outcome, cluster count, inter-centroid cosines); (4) the whole-meeting candidate list under the FULL gate set (onset detection included), mapped to every entry whose span OR pin window intersects [gap.start − gap.len, gap.end + gap.len], floor margins recorded, evaluated against the post-re-pin fixture; (5) controls — true-silence gaps under the same gates (false-rescue denominator) + the ±0.2s padded variant; (6) post-splice predicate rehearsal — replicate the piece list, splice at the onset, re-run resolve_turns, assert S2b/S1/S3 predicates. GO requires: correct decided vote, onset found ≈15.8, no candidate disturbing any entry other than the intended improvement, bounded false decisions, rehearsal green.
