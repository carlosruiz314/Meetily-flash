## Why

Ear round 2 (2026-10-06) found four render-fidelity defects the golden-fidelity gate structurally cannot see, and the explore-stage adversarial panel (2026-10-06, 5 reviewers) found a fifth nobody knew about: **six sub-second synth pairs carry physically impossible decodes** (up to ~35 words/s against their wall duration — Whisper hallucinating fluent template sentences on sub-second separated streams), rendering as machine-fiction rows under real speaker badges today, including inside the S16 showcase window whose attested needles are actually carried by a plain row. Root cause (panel consensus): the gate validates synthesis *decisions* — order-free, multiplicity-free, badge-blind word-presence checks — not render *fidelity*; its recall on the ear round was 0/4. Each landed fix raised trigger recall while the splice shape stayed lossy, moving defects from trigger space into shape space (whack-a-mole). Now — while the ear-round evidence is fresh and the defect classes are freshly mapped — is the moment to make fidelity mechanical so ears rule classes, never moments.

## What Changes

- **Gate fidelity assertions (the core)**: needle assertions become multiplicity- and badge-aware (exactly once, under the attested badge — "presence-only" passes die); a boundary-token scan flags synth-row tails sharing ≥3 tokens with temporal-neighbour heads (leak fragments — the current 80%-of-shorter-row duplicate bar structurally misses these); a words-per-second sanity bar rejects decodes exceeding human speech rate against their row walls (class E); a parent-link integrity scan (dangling `synth_parent` semantics pinned, wall containment asserted); the fracture/overlap scans' same-donor exemption is narrowed so it stops being vacuous for the shapes the splice produces.
- **Synthesis rate floor**: sub-second separated streams (below a pinned duration floor, with the words/sec bar as backstop) no longer synthesize — the real words live in the mixed rows they replace, so standing down loses no attested text while removing machine fiction.
- **Decode-closure generalization**: `decode_span_synthesis` takes the decode closure so the gate replay's two inline decode loops collapse into one — removing the three-way lockstep hazard that made every synthesis change a divergence risk.
- **Census-replay harness (fast-fail tier)**: synthetic, token-only census fixtures drive the REAL pure splice functions in milliseconds (`cargo test`, no GPU), pinning StreamEvidence inputs + fake-decoder contracts; the live gate stays the real-model parity pin. Ladder: unit → warm replay → span probe → cold gate.
- **Data-driven ear pins**: ear-dictated turn sequences become ordered per-row needle pins in the fixture (a new entry kind), replacing hand-coded assertion blocks — every dictation permanently upgrades the gate.
- **Token census (data-first, task 4.4's phase A)**: the span decode gains a sibling adapter method returning token timestamps (decode text byte-stable, sha256 cross-check) recorded as census evidence for phrase onsets; NO shape change until wall reliability is ear-validated.
- **Edge absorption**: repair windows may absorb the edge row WHOLE under a separate repair-chain cap (the 30s mass-span guard stays); never partial coverage.
- Deferred to ear rulings: any flag graduating to an automatic veto, and any intra-row shape change (both recorded as explicit graduation paths, never defaults).
- **Class B (chronology flattening) coverage is honest, not full**: the interim mechanism is the ordered per-row pin schema — the clip-04 dictation lands as a pin documenting the flattened render (a recorded KNOWN-LIMITATION until token walls land in M2/M4). The general mechanism is token-timestamp walls, ear-gated; this change does NOT claim to fix class B mechanically.
- **Archive-order dependency**: this change's MODIFIED requirement blocks are derived from the in-flight `overlap-stream-retranscription` delta and assume that change archives FIRST; the blocks MUST be re-derived from the live spec immediately before this change's archive (they are word-for-word copies today; the risk is temporal, not textual).

## Capabilities

### New Capabilities

- `render-fidelity-verification`: the mechanical fidelity layer — words-per-second sanity, boundary-token leak detection, multiplicity/badge-aware needle matching, parent-link integrity, and the census-replay harness that verifies render shape in milliseconds without GPU or real meeting data.

### Modified Capabilities

- `speaker-diarization`: synthesis acceptance gains the rate floor and the boundary-fragment evidence requirement; the S16/S18 render requirements (S18 = the 172.01–199.50s fabrication window) change from presence-only needles to multiplicity/badge-aware per-row pins; repair-window chaining gains the separate edge-absorption cap; a pin kind asserts synthesis stand-down (a span expected to render mixture rows only).

## Impact

- `frontend/src-tauri/src/audio/speaker/run_assembly.rs` (splice gates, new pure scan fns, rate floor), `commands.rs` (`decode_span_synthesis` closure generalization; new token-census adapter method in the Whisper surface), `whisper_engine/` (sibling token-returning decode), `tests/ear_truth_gate.rs` (assertion upgrades; decode-loop collapse), new `tests/` census-replay harness + fixtures, `openspec/changes/.../specs/` deltas for both capabilities.
- Gate-run cost unchanged (~70 min cold); the replay harness adds a milliseconds tier that must have caught the three burned gate runs of 2026-10-05 by design.
- Fixture bright line respected: repo-committed harness fixtures are PURELY synthetic (invented walls/word counts, engineered defect shapes — never derived from real meeting spans); meeting-shaped fixtures live in `MEETILY_LOCAL_EVIDENCE_DIR` (the pre-push guard's bright line covers meeting-derived synthetic data too).
- Ear-round economics: clips 07–12 drop out (class E is ruled by physics); future ear rounds become class-stratified with agent pre-triage.
- No breaking changes to persisted row shape in M1/M2; M3/M4 shape changes are gated on ear rulings by construction.
