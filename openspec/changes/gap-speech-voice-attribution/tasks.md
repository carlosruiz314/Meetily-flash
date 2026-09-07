## 0. Land the base first (ONE commit, re-pins included)

- [x] 0.1 Apply the user-confirmed 2026-09-07 re-pins to the fixture: S1 → `[9.38, 11.8]` single_voice; S2 → `change_at 12.0±0.75`, span `[11.8, 13.5]` (expected-fail: engine boundary 13.42 is 1.42 off — missed "Yeah" change, separate follow-up class); S2b → `change_at 15.8±0.75`, span `[15.0, 16.8]` (passes TODAY on the current engine; note records the rescue's win is text attribution); notes carry the clip_C/D/E basis
- [x] 0.2 Add S2 (not S2b) to `known_limitations` (inline reason: sub-run change miss at ≈12.0, user-ear-confirmed via clip_D 2026-09-07, fix owned by a follow-up change), re-run the ear gate to green (S2 KNOWN-LIMITATION; S2b PASSING), then commit the whole change-set as ONE commit — a two-commit split has no green intermediate (S14's pin moves 162.78 → 161.36, |Δ| = 1.42 > ±1.0). Base hash: `e237eea64ab9de442964b4fe4baad6fec9e6433a` (2026-09-07). Nothing is pushed without explicit user sign-off

## 1. De-risk probe (GO/NO-GO before engine code)

- [x] 1.1 Ignored probe (reuse `closure_gap_probe` skeleton): current post-trust-zone turn set for 12–17s and 32–38s, pyannote per-window votes for BOTH windows (putting the gap's suppression confidence on the committed record), measured borrow distances at the target gap; log to `openspec/exploration/`
- [x] 1.2 Same probe: VOICED-ONSET detection rehearsal inside 14.78–16.12 — energy/voice-activity profile of the gap; does onset detection find ≈15.8 (the mechanism depends on it)? Record the detector parameters used
- [x] 1.3 Same probe: identity votes for BOTH raw windows — [14.78, 16.12] (gate `transcripts.json` mega-row source) and [14.78, 15.84] (production DB fine row) — plus the voiced sub-window, with the production reference set; report margin, best cluster, `is_effectively_silent` outcome, cluster count, inter-centroid cosines
- [x] 1.4 Same probe: whole-meeting candidate list under the FULL gate set (embedding ∧ raw span∩gap ≥ 0.8s ∧ margin ≥ 0.05 ∧ distinct-turn gap ∧ best ≠ modeled borrow winner ∧ onset detected), evaluated against the post-re-pin fixture; each candidate mapped to every entry whose span OR pin window intersects [gap.start − gap.len, gap.end + gap.len], floor margins recorded
- [x] 1.5 Same probe controls: true-silence gaps under the same gates (false-rescue denominator) + the ±0.2s padded-span variant of the target window
- [x] 1.6 Same probe: post-splice predicate rehearsal — replicate the piece list via the pub run_assembly functions, splice the decided candidate AT THE ONSET as a `PieceIn`, re-run `resolve_turns`, assert the S2b / S1 / S3 predicates on the resulting turns (makes GO predictive of the 4.4 gate run)
- [x] 1.7 **NO-GO recorded 2026-09-07** (log: `openspec/exploration/gap-rescue-probe-20260907-full.log`): (a) RAW-window identity votes CARLOS (sp0 margin 0.226/0.242) — the gap's 14.78–15.8 stretch is Carlos's tail (onset detected 14.78, peak −29.8 dBFS), so raw-span identity + first-onset placement would splice a confidently-wrong Carlos piece; (b) only the ear-positioned voiced window [15.8,16.12] votes Cynthia (0.096, decided); (c) true-silence controls show decided margins up to 0.14 (margin alone is not protective); (d) replica DRIFT (423 vs 222 turns — shed-to-cap sub-floor merging not replicated) invalidates the piece-level rehearsal. **PIVOT v3**: segment the raw span by energy into voiced sub-windows; identity PER SUB-WINDOW; attribute the row to the LAST decided sub-window (skew-aware: rows skew early, words sit at the row tail); splice = that sub-window's span; re-scan candidates under v3 before engine code.

## 2. Pure candidate selection (`run_assembly`)

- [ ] 2.1 RED→GREEN: `rescue_candidates` selects a distinct-turn text-bearing gap with a decided, borrow-winner-contradicting identity and a detected onset (synthetic arrays; asserts span = [onset, gap end], ordered inputs, no hash maps in the decision path)
- [ ] 2.2 RED→GREEN abstain/no-op branches: no embedding, sub-floor intersection (< 0.8s raw), margin below gate, interior gap, best == modeled borrow winner, single-cluster meeting, meeting-edge gap, NO CONFIDENT ONSET → abstain (untrimmed row-start splice forbidden); and the splice sides: both flanks beyond borrow cap → splice when decided, far-flank match splices
- [ ] 2.3 RED→GREEN: multi-row gap yields ONE candidate (union span); borrow-winner rule (midpoint reference, containment 0, i64 ms, tie → earlier turn); determinism (same inputs twice → identical candidates)

## 3. Engine wiring (`run_engine`)

- [ ] 3.1 RED→GREEN (or gate-verified): `derive_turns_from_masses` runs resolve_turns once (pre-rescue turns), derives silence gaps + union spans, detects voiced onsets (signal-level energy relative to the gap's own level), embeds after centroid finalization (reusing the `embed_slice` convention), splices selected candidates at their onsets in one time-ordered pass, and runs resolve_turns again; debug `PIECE` zip fixed for synthetic pieces
- [ ] 3.2 Zero-cost guard: gaps with no overlapping text span never touch the extractor (embeddings bounded by the text-bearing distinct-turn gap count, one per gap)
- [ ] 3.3 `cargo test --lib` green including new units

## 4. Fixture + gate

- [ ] 4.1 Fixture amendment schema: `amendments: BTreeMap<String, Amendment { user_confirmed, reason }>` (serde default) on `Fixture`; waiver rule = id ∈ `known_limitations` AND complete amendments record → gate prints `AMENDED(<date>, <reason>)` and counts it limited; non-live fixture-lint unit test fails any `known_limitations` id without a complete record and rejects orphan amendment records; 0.2's inline reason migrates into the schema
- [ ] 4.2 Full chain green via `.bat` runners: lib, synthetic 3/3, ear gate zero FAIL (S2 the sole KNOWN-LIMITATION; S2b PASSING before and after) with render gate clean; evidence logged to `openspec/exploration/`; RENDER line diffed against the recorded baseline (`trustzone-gate-20260906-round2-15of15-render.log`: 237 → 531 → 422) with any fragment-count jump beyond the rescued spans' own rows investigated
- [ ] 4.3 Render-text acceptance: the persisted fragment containing "Oh, man" carries Cynthia's label (the rescue's user-visible win), not just the engine-side turn set

## 5. Verification

- [ ] 5.1 Adversarial review pass on the diff to convergence (verify-before-implement on findings)
- [ ] 5.2 User performs the single Speakers re-run covering base + rescue; "Oh, man" renders under Cynthia; the "Yeah" region stays as-is (known miss, follow-up change)
