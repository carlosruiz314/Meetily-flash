## Context

Measured 2026-09-08 (exploration log `render-print-20260908-b.log` — the authoritative replay: production align → borrow → merge → consolidate over live DB rows): the persisted row shape after a Speakers run on cde5c264 shows 163 of 240 rows beginning mid-sentence (raw lowercase count; the cross-badge fracture subset is what the gate asserts — see D5). Engine turn boundaries pass the ear pins (15 PASS, 1 known limitation, 0 turn-level violations), but row TEXT is divided at those boundaries and the halves land under different badges ("Where is | Ricardo | I | don't know. Let me ping in...").

Panel-measured ground truth (adversarial round 1, 2026-09-08; production DB read-only):

- **237/240 rows have NULL `token_timestamps`.** The 3 non-null rows are hallucination garbage (mojibake text, ~11 kB token blobs over ~1–2 s spans). Token-precise alignment is effectively a NON-PATH on this fixture; proportional spans are the default regime. (The stale CAVEAT in commands.rs claiming NULL-for-every-row predates these 3 rows.)
- **The duplicate instance is a 3-row chunk-overlap re-transcription cluster**, not an equal-text pair: [74.28] "for motors, we're doing the feature flag update. So for" / [80.34] "motors, we're doing the feature flag update. So for motors," / [85.49] "we're doing the feature flag update." — complementary edges of one re-decoded utterance, disjoint spans, 0.4–1.1 s gaps. Badges are unstable across replays (Speaker 0/1 in DB; Speaker 1/Unknown in the replay) — the predicate must not pin badges. 0 adjacent equal-normalized-text pairs exist meeting-wide.
- **The live DB mutates underneath the gate**: all 240 rows now carry labels (a Speakers run executed 2026-09-08 ~13:39, after the replay capture). The Enhance button ALSO auto-runs diarization (`retranscription.rs` post-pass, gated only by `settings.diarization_enabled`), as does the rename flow's reference pool. The gate as written reads whatever rows are live, can silently pass on 0 rows, and its RED baseline is unreproducible after any run.

Root cause chain (unchanged): engine turn boundaries are voice boundaries, not sentence boundaries (and must not be — voices interject mid-sentence); alignment divides row text at every flip; same-speaker merge/consolidation cannot rejoin fragments across a flip.

Earlier same-day evidence (`render-print-20260908.log`, 246/423) replayed the sidecar's token-less proportional path and misassigned "Yeah. That's right."; the DB-row replay above is the production truth and supersedes it.

## Goals / Non-Goals

- Goal: a persisted row never shows half of a sentence under a badge that did not speak the other half (cross-badge fracture = 0, gate-asserted). The user's verbatim rule: "Do not come back to me if you have split sentences across speakers... Any sentence-speaker segment starting with a lowercase letter is an immediate suspect." The SUSPECT rule is a screen; the hard defect is the cross-badge split. Lowercase onsets that are the ASR text itself are not fixable by assignment and are explicitly out of scope.
- Goal: the ear gate asserts this meeting-wide over a SNAPSHOT-pinned input, reproducible across live-DB mutations.
- Non-Goal: changing engine turn boundaries (owned by gap-speech-voice-attribution and the parked sub-run change).
- Non-Goal: re-punctuating or re-casing text (mechanical-only doctrine).
- Non-Goal: general token-glue repair ("Paul ina", "spr ints") — EXCEPT the minimal case segmentation depends on (D1: ≤2-char tail atoms).

## Decisions

### D1: Sentence is the assignment atom — with proportional spans as the default regime

**Fragment rejoin first**: the pipeline input (and the snapshot fixture) contains persisted fragment rows already split at turn boundaries ("I" is its own row), so per-row segmentation alone cannot repair the motivating fractures. Assembly first rejoins ADJACENT persisted rows whose predecessor lacks sentence-terminal punctuation into one logical text unit (text-level repair across badges and row gaps — word order and content preserved, not a badge merge; this generalizes the tiny-tail case below). Segmentation then runs on the logical units.

Span source: valid token timestamps (per-sentence first/last word) ONLY when the row's token JSON passes a sanity clamp (≤25 tokens/second — normal speech is ≤~6; the three hallucinated ~11 kB blobs are thousands) AND the token-word list count matches the row's whitespace tokenization; otherwise proportional shares of the unit's span (the 237/240 regime — this is the primary path, not an edge case). When tokens are valid, segmentation runs on the whitespace-joined token-word list (the same list the spans come from), never on the DB text separately.

Segmentation rules (each unit-tested):
- Sentence terminators: `.?!` plus full-width `。？！` and `…`/`...` NORMALIZED to a single terminator pre-split; atoms with no alphanumeric character are dropped (never emitted).
- Assignment is whole-atom by majority: a short straddling sentence ("Where is Ricardo?" across a spurious flip) is ONE atom assigned to the majority badge — never cut, never fractured.
- **No bounded-run guard (user ear decree, 2026-09-09 — supersedes the earlier 8 s cut design)**: the voice does NOT change mid-sentence in the user's meetings, so every engine boundary inside a sentence is an engine error, absorbed by whole-atom majority assignment. The bounded-run cut, its `cross_badge_tail` flag, and the waiver class are RETIRED — there is NO waiver for cross-badge fractures; the gate fails hard on every one. The fix for a wrong badge is engine boundary accuracy (see `engine-boundary-and-identity-accuracy`), never cutting the sentence.
- **Tiny-tail glue**: a ≤2-character tail atom ("y." from "Oka|y.") merges into the previous atom when the previous row lacks a terminator, before any assignment — a special case of the rejoin pass, taking the rejoined unit's badge by majority.

Assignment scorer (deterministic, unit-tested on ties): maximize `overlap_ms` between the sentence's span and the engine turns; tiebreak = edge distance from the sentence MIDPOINT to the turn edge (0 when contained, mirroring `nearest_turn_span`'s convention); a near-tie (within 100 ms) prefers the PREVIOUS atom's badge (temporal contiguity).

**Reattribution, not a merge** (reconciles sentence-aware-turn-assembly's "Rows of DIFFERENT speakers SHALL never merge"): minority-span words inside a majority sentence move with their sentence to the majority badge; no row combines two speakers' words; the engine turn set is untouched; a turn whose span holds no assigned sentence emits no row (existing textless-turn doctrine). This is the horn the panel demanded be picked explicitly.

### D2: Backchannels keep their own badge

A short interjection sentence ("Yeah,") between one speaker's sentences stays its own row (per sentence-aware-turn-assembly's interjection rule and the D9-amended engine-turns-are-final doctrine recorded in run_diarization_for_meeting Step 8). Host sentences assign by D1. Cross-badge merges are forbidden at segmentation (D1), so the host sentence cannot render under the interjector's badge. Churn is MEASURED, not guessed: the gate reports persisted rows/minute and rows of ≤2 words; threshold decisions deferred until GREEN (panel: 16 short question-atoms exist meeting-wide).

### D3: One owner for silence borrowing

Align emits "Unknown Speaker" ONLY for sentences overlapping no turn (per the canonical no-borrow tail rule). The EXISTING `assign_engine_gap_fragments` (midpoint, edge distance, cap 3000 ms) remains the sole cap-borrow stage — task 2.3 adds NO fallback of its own; D3's earlier "borrow-cap fallback inside assignment" is deleted (two competing nearest-metrics made Unknown nondeterministic).

### D4: Duplicate clusters are merged with span union — never silently dropped

Predicate (all must hold): normalized token sequences share a contiguous subsequence of ≥3 tokens covering ≥80% of the shorter row; spans disjoint (overlapping spans = same-audio double-decode — NOT dropped: reported via the gate's `overlapping-span rows` counter and a render_failures entry); inter-row gap ≤ 2 s; different badges or one side "Unknown Speaker". Normalization = `detokenize` → lowercase → non-alphanumeric → space → collapse whitespace. Resolution: keep ONE copy — survivor badge: labeled over Unknown; among multiple labeled rows, the badge owning the majority of the cluster's union span; final tiebreak earliest start (deterministic, unit-tested) — write text ONCE (never concatenate the duplicate's text — it is already present), extend the survivor's span to the UNION of the cluster's spans, delete absorbed shells in the persist transaction, and delete/absorb the source rows explicitly — `persist_aligned_splits` bails on an empty group without touching the source row, so a naive drop orphans a NULL-labeled full-text row and the defect survives. The dropped duplicate's time is preserved via the union (coverage never shrinks — mirrors `consolidate_meeting_turns`' UPDATE-then-DELETE shape). Single contract (no gate/persist contradiction): assembly resolves duplicates; the gate asserts the persisted shape contains 0 duplicate clusters, and a unit test pins the resolution itself. The gate also scans a bounded window (±10 s), not strict adjacency — the motivating cluster is not adjacent in the persisted sequence.

### D5: The gate asserts cross-badge fractures over a snapshot-pinned replay

- **Fracture predicate** (the assertion; replaces the unsatisfiable zero-lowercase target): row i>0 violates iff `is_mid_sentence_start(text_i)` AND `badge_i != badge_{i-1}` AND row i−1 lacks terminal punctuation after trimming closing quotes/brackets. Row 0 exempt. Same-badge lowercase starts need no marker: consolidation already merges same-badge ≤3 s neighbors, so a surviving same-badge adjacency is a >3 s resume whose lowercase onset is ASR style. Render-level failures (duplicate, unknown-within-cap, zero-dur, unmerged) may carry the amendment-record waiver path mirroring fixture entries; the FRACTURE scan does not — since the 2026-09-09 ear decree every cross-badge fracture fails hard, with no waiver kind (matches `record_render_failure`'s updated contract in the gate).
- **Snapshot fixture**: the 240 input rows (id, text, start/end ms, token JSON) are copied to `tests/fixtures/cde5c264_transcripts.json` with a SHA-256 over the ordered rows and the FULL meeting id. The gate replays from the snapshot and cross-checks the live DB (exact-equality meeting lookup, assert exactly one match, assert count+hash) — mismatch hard-fails with "fixture drifted — re-pin", never vacuous green. Every Speakers run rewrites the row set (delete+insert), so without this the RED baseline is unreproducible.
- **Known pre-existing flaw, recorded**: the turn-level "HARD INVARIANT" scan is tautological (`effective_continuation` = `engine_flag || is_mid_sentence_start(text)` — true whenever its own antecedent holds). Fixing it is out of scope here; task 1.4 annotates it so its "0 violations" is not cited as evidence.
- **Re-pin checklist**: under sentence atoms the RENDER-TEXT assertion (15.7–16.0 s window), `unknown_within_cap`, zero-dur, and unmerged checks all re-judge reshaped fragments — task 3.1 re-verifies each explicitly instead of assuming survival.

## Sequencing (binding)

1. gap-speech-voice-attribution task 4.2 (replay-only diff) executes BEFORE any no-split gate run with assertions; its recorded baseline (sidecar, 237→531→422) is superseded by the 09-08 DB-row replay (240→284→243) — reference the new log.
2. RED→GREEN entirely on the snapshot; no live gate signal in between is valid.
3. ONE combined user-gated live run on cde5c264 verifies gap-speech 5.2 AND no-split live checks simultaneously (both tasks files cross-reference it).
4. Freeze until archive: no Speakers click, no Enhance on cde5c264 (Enhance auto-runs diarization), no badge renames (mutates the enrolled-reference pool the gate loads).
5. Archive AFTER sentence-aware-turn-assembly; task 3.4 verifies its "Persisted speaker turns are sentence-readable" requirement exists in the main spec and that this change's reattribution clause (D1) reconciles with its no-merge rule before archiving.

## Risks / Trade-offs

- [Proportional sentence spans are approximations (237/240 rows)] → majority-overlap can misassign a sentence hugging a flip; bounded by the midpoint tiebreak and measured meeting-wide by the gate rather than trusted.
- [Reattribution moves a minority speaker's words under the majority badge] → this is the user-accepted doctrine (sentence follows its speaker); the engine turn set still records the flip; content is preserved (D4 union + D1 atoms).
- [Duplicate predicate could catch a genuine repeat] → requires ≥3-token contiguous cross-badge match with disjoint spans ≤2 s apart; genuine repeats ("I can't. I can't.") live inside one row or are full-sentence interjections and do not match; unit tests pin both sides.
- [Sentence-atom rows change consolidation's input granularity] → the 3 s same-speaker gap threshold may over/under-merge; deferred, measured after GREEN (Open Questions).
- [Fixture snapshot goes stale by design after any live run] → intended: staleness must be LOUD (hash mismatch), not silent.

## Migration Plan

1. Snapshot the 240 input rows + hash into the fixture; harden the gate's meeting selector.
2. RED: fracture + duplicate assertions with the shipped predicates; measure and pin the true RED counts (NOT the 163 raw-lowercase number).
3. GREEN: D1 segmentation + assignment in alignment; D4 duplicate resolution at persist; rewrite the existing split-pinning tests (`token_alignment_splits_multi_speaker`, the A→B→A three-way test, `proportional_cjk_no_whitespace_is_divided`, and the alignment tests enumerated in tasks 2.4) to sentence-atom expectations.
4. Gate to zero fractures / zero duplicate clusters on the snapshot; re-verify the render-text re-pin, unknown-within-cap, zero-dur, unmerged; ear entries stay 15 PASS / 1 known limitation / 0 FAILED.
5. One combined live run (user-gated), then archive after sentence-aware-turn-assembly.

## Open Questions

- Whether the 3 s same-speaker consolidation gap should change under sentence-atom rows (defer; measure after GREEN).
- Churn threshold for ≤2-word rows (measured first; decide later).
