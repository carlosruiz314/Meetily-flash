## Context

The 2026-09-07 live render exposed the last defect class the trust-zone work could not see: speech pyannote decodes as silence. In the cde5c264 region 9.38–20.3s the engine holds turns `9.38–13.03 sp1` / `13.42–14.78 sp0` / `[hole 14.78–16.12]` / `16.12–20.30 sp1`. The hole is the "Oh, man" case: pyannote confidently suppresses real speech there, the row `14.78–15.84 "Oh, man"` overlaps no turn, and production's render borrow gives it to Carlos's badge — Cynthia's word under the wrong speaker (user-ear-recorded 2026-09-07).

The user then ear-adjudicated the whole region with clips C/D/E (verbatim answers in `check_clips_2026-09-06.json`, confirmed pins):

- **Change Cynthia→Carlos at ≈12.0** ("Yeah", clip_D: "definitely around 0.5s not 1.9s"). The engine misses it — its 13.42 boundary is 1.42s off. This is a SUB-RUN change miss (the stretch decodes as speech), outside the gap-rescue mechanism → recorded as the S2 expected-fail known-miss, fix in a separate follow-up change.
- **Cynthia's "Oh, man" starts ≈15.8** (clip_E: "around 1.5s"). The engine's existing 16.12 boundary is within 0.32 of that — **S2b PASSES TODAY**. The known-miss dissolves; what remains broken is the text attribution ("Oh, man" renders under Carlos).
- Legacy row timestamps in this region are locally skewed in BOTH directions (ear-attested): "five years."/"Yeah." ~1.4s late, "Oh, man" ~1.0s early. Consequence: the rescue MUST place its splice at the detected voice onset inside the gap (≈15.8), never at the raw row start — a row-start splice would move the boundary to 14.78 and REGRESS the now-passing S2b.

Engine-side facts that make the rescue tractable (all verified in the working tree): final centroids and the `best_centroid` machinery exist downstream of clustering; `text_spans` is already a parameter of `derive_turns_from_masses`; `resolve_turns` natively coalesces same-cluster neighbors, maps `promoted_subfloor → low_confidence = true`, and derives `continues_previous` from the preceding turn's cluster; the render borrow's winner rule is the fragment midpoint with point-to-span distance and time-order tie behavior (`commands.rs:919-920`). Transcript geometry differs by source — the production DB's fine row gives raw span∩gap `[14.78, 15.84]` (1.06s), the gate source's coarse mega-row `[14.78, 16.12]` (1.34s); both clear the 0.8s floor and both pick the same borrow winner.

Panel history: rounds 1–3 converged the mechanism (5+3+2 reviewers); round 4 of ear adjudication (clips D/E) then revised the pins, which this version absorbs.

## Goals / Non-Goals

**Goals:**

- The S2b class — Whisper-heard speech in a pyannote-silence gap between two distinct turns — gets attributed by voice with the splice placed at the detected VOICE ONSET, so the created boundary lands where the ear says the voice starts, not where a skewed row starts.
- The user-visible win is rendered text: "Oh, man" renders under Cynthia. S2b passes before AND after (no regression).
- Strict fall-through: every non-rescued span, and every candidate that fails any gate (including onset detection), behaves exactly as today.
- Pure, deterministic candidate selection in `run_assembly`; model I/O stays in `run_engine`; snapshot semantics (candidates evaluated against the pre-rescue turn set, spliced in one shot, final resolve).
- Gate-verified end-to-end: zero FAIL outcomes with S2 (@12.0) as the recorded, user-signed KNOWN-LIMITATION.

**Non-Goals:**

- No change to the pyannote pass, speech-gate thresholds, piece derivation, clustering, resolve_turns semantics, or the render/borrow/merge layer.
- No fix for the S2 miss (sub-run change at ≈12.0 inside decoded speech) — different mechanism, separate follow-up change; documented as the gate's known-limitation.
- No rescue for gaps interior to a single turn, meeting-edge gaps, or the "Gotcha" class (no transcript row → no trigger; transcription-coverage issue).
- No overlap/crosstalk disentanglement (the margin gate abstains on mixtures).
- No enrollment dependency — rescue uses the meeting's final centroids as derived; margins are not comparable across centroid-count regimes, so the probe records cluster count + inter-centroid cosines and calibration (if any) happens under gate governance.
- No UI for `low_confidence` (engine/gate-layer only today — dropped in the `SpeakerSegment` mapping, never persisted). A candidate coalescing into the PRECEDING flank leaves that flank's flags unchanged; only a candidate that founds a turn carries the promotion flag.

## Decisions

- **Voice-onset splice placement, trim-or-abstain.** The engine detects the voiced onset inside span∩gap by signal energy relative to the gap's own level, and the spliced piece starts there (ending at the coalescing gap edge). If no confident onset is detected, the candidate abstains — an untrimmed row-start splice is FORBIDDEN (it would place the boundary at a skewed timestamp and regress S2b, which passes today). The onset detector is signal-level (energy/voice-activity), not a model call.
- **Embed window = the raw span∩gap** under the `embed_slice` convention (floor-guarded text evidence); the probe measures whether identity improves on the voiced sub-window, and any switch is calibrated under gate governance — the voiced sub-window (≈0.3–0.5s here) must NOT lower gate 2's floor.
- **Synthetic promotion-floor pieces, not a post-hoc turn splice.** Selected candidates splice into `piece_ins` in time order as `PieceIn { cluster: Some(best), margin, promoted_subfloor: true, span = [onset, gap end] }` before a SECOND `resolve_turns` pass (the pre-rescue output is what the candidate gates evaluate against). Far-flank matches coalesce; both-flank-differing candidates stand as their own low-confidence turns; `continues_previous` derives natively. The debug `PIECE` zip is fixed as part of wiring.
- **Candidate gates (all verified against the working tree):** embedding exists (per-gap failure abstains; model-load failure fatal, unchanged) ∧ raw span∩gap ≥ 0.8s (`PROMOTION_FLOOR_SECS`; native promotion case, no doctrine exception) ∧ decided margin ≥ AMBIGUITY_MARGIN (0.05) ∧ gap separates two DISTINCT turns (adjacency-based membership) ∧ best ≠ modeled borrow winner (midpoint of span∩gap in i64 ms, point-to-turn-span containment-0, ties → earlier turn, within GAP_BORROW_MAX_MS) ∧ voiced onset detected. Single-cluster meetings abstain. Neither flank within the cap → splice when the other gates hold. Multi-row gaps yield ONE candidate: span = union of intersecting row intervals clipped to the gap; one embedding per text-bearing distinct-turn gap.
- **Re-pinned fixture (user-confirmed 2026-09-07, clips C/D/E):** S1 → `[9.38, 11.8]` single_voice (passes before and after); S2 → `change_at 12.0±0.75`, span `[11.8, 13.5]` — EXPECTED-FAIL (engine boundary 13.42 is 1.42 off), the recorded KNOWN-LIMITATION; S2b → `change_at 15.8±0.75`, span `[15.0, 16.8]` — PASSES TODAY (engine 16.12 within 0.32) and MUST pass after. S2b's entry note records that the rescue's win is text attribution, not the boundary.
- **Fixture amendment machinery (schema pinned).** `Fixture` gains `amendments: BTreeMap<String, Amendment { user_confirmed, reason }>` (serde default). Waiver rule: an entry fails the assert UNLESS its id is in `known_limitations` AND a complete amendments record exists; the gate prints `AMENDED(<date>, <reason>)` and counts it limited. The fixture-lint unit test (plain `cargo test`) fails any known_limitations id without a complete record and rejects orphan records for passing entries.
- **Task 0 is ONE commit including the re-pins.** The uncommitted set + the user-confirmed pin updates (S1/S2/S2b) + S2's KNOWN-LIMITATION waiver land as a single commit, gate re-run to green first (S2b passes on the current engine under the new pin; S2 is the only limitation). The previously proposed two-commit split cannot be gate-green in either order (S14's pin moves 162.78 → 161.36, |Δ| = 1.42 > ±1.0).
- **De-risk probe before engine code** (GO/NO-GO), reusing the `closure_gap_probe` skeleton: (1) current turn set + pyannote votes for 12–17s and 32–38s on the record + the measured borrow distances at the target gap; (2) VOICED-ONSET detection rehearsal inside the 14.78–16.12 gap — does energy-based detection find ≈15.8? (the mechanism now depends on it); (3) identity votes for the raw span∩gap window [14.78, 15.84] AND the voiced sub-window, from BOTH transcript sources with the production reference set, reporting margin, best, silence-gate outcome, cluster count, inter-centroid cosines; (4) whole-meeting candidate list under the FULL gate set (including onset detection), mapped to every entry whose span OR pin window intersects [gap.start − gap.len, gap.end + gap.len], floor margins recorded; (5) controls: true-silence gaps under the same gates (false-rescue denominator) + padded-span variant; (6) post-splice predicate rehearsal — replicate the piece list via the pub run_assembly functions, splice the decided candidate AT THE ONSET, re-run resolve_turns, assert S2b/S1/S3 predicates. GO requires: correct decided vote, onset found at ≈15.8, no candidate disturbing any entry other than the intended improvement, bounded false decisions, rehearsal green.

## Risks / Trade-offs

- [Onset detection misplaces the splice] → trim-or-abstain: a missed onset means abstention (status quo, S2b still passes via 16.12); a ±0.3s onset error stays inside S2b's ±0.75 boundary pin. The TEXT-attribution win has a tighter budget: it requires onset < the row end (15.84) so the merged turn overlaps the row — a late onset inside the pin degrades the boundary correctly but leaves the borrow's status-quo badge, which task 4.3's render-text acceptance catches. The probe rehearses detection before engine code.
- [Identity on the silence-diluted window] → probe measures both windows; the voiced sub-window is 0.3–0.5s (the marginal regime) — if the diluted window votes wrongly and the voiced window votes rightly, the design switches identity to the voiced window under gate governance (placement and identity are independent knobs).
- [S2's 12.0 miss sits 1.42s from the engine boundary — a user-visible misattribution ("Yeah" under Cynthia) that this change does NOT fix] → documented as the gate's KNOWN-LIMITATION with user sign-off; follow-up change owns it (sub-run change detection class).
- [Floor-riding candidates (0.81s/0.82s gaps clear the floor by ≤20ms)] → probe lists them with floor margins; deterministic gates are stable for fixed inputs.
- [Boundary-adjacent disturbance (S12 class)] → probe's pin-window mapping catches it before engine code.
- [Margin is not scale-invariant across centroid sets] → probe records cluster geometry; calibration under gate governance, hold-out entries excluded.
- [Digital-silence-only guard admits breath/room-tone embeddings] → the margin gate is the substantive protection (the "free hallucination guard" overclaim was corrected in round 1).
- [Pre-existing, noted: the gate's continuation-marker scans cannot catch flag lies — hand-traced clean instead.]

## Migration Plan

No data migration. Task 0 lands the uncommitted set + user-confirmed re-pins as ONE commit (S2 as KNOWN-LIMITATION, gate green); this change lands on top; the user performs ONE Speakers re-run covering everything.

## Open Questions

- Identity window: raw span∩gap vs voiced sub-window — probe decides; gate governance governs.
- Onset detector parameters (energy threshold relative to gap level, minimum voiced run) — probe rehearsal decides; if unreliable, the change degrades to abstain-only (no rescue fires) and the design is revisited.
