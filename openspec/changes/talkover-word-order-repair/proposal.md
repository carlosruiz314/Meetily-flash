## Why

Talk-over spans render the mixture decode: correct words exist but their order is jumbled (two voices interleaved in one row). The previous repair attempt (`overlap-repair-net-worse-guard`, archived 2026-11-17) proved the separated decodes CAN recover the true sequence, then dropped the promotion because its automated acceptance bar (token-conservation net-worse guard) was falsified by measurement — no conservation ratio separates a validated render from a paraphrased one. The user rejects the limitation and has ruled the path (2026-11-18): **make separated decodes reliable first, then ear-attest each repaired span** — decode reliability removes the failure rate the falsified bar tried to catch, and dated ear pins become the acceptance oracle the automation could never be.

## What Changes

- **Decode-reliability stage (offline, no render change):** widen the repair-window context and add a decode retry profile (deterministic greedy decode, no-condition retry on rate-fiction/stutter) so separated-stream decodes recover verbatim text on the attested window shapes; measured offline against the evidence-home fixtures before anything ships.
- **Candidate-flag census (token-only, env-gated):** record every overlap-span candidate flag with its detector evidence (overlap mass, duration, which fire the floors) so detector precision is measured instead of assumed — answers "is it flagging talk-overs that do not exist".
- **Promotion rebuild (~200 lines, from the archived rebuild-notes):** a qualifying talk-over span promotes as a repair seed and routes through the LIVE repair-window builder (separation + per-stream re-decode + per-voice splice) — the machinery that survived the drop unchanged.
- **Acceptance = ear pins, not a mechanical bar:** the automated layer only REJECTS (existing hallucination guards, rate-fiction/stutter stand-down, margin-gated votes, distinct-voice requirement, manual-row-wins, byte-identical degrade). It never ACCEPTS. A promoted render ships only after the user's dated ear verdict becomes a fixture pin; until a span is pinned, its region keeps the mixture rows (fail-closed).
- **Per-span attestation flow:** for each fired span, cut a before/after clip (evidence home or meeting folder ONLY), produce a token-only manifest, and record the user's dated verdict as a pin the ear gate enforces forever — via the existing waiver-router/pin machinery, no new gate code shape.

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `speaker-diarization`: the per-voice overlap-render requirement gains the promotion path (seed → decode-reliability profile → per-voice splice), the acceptance rule (ear-pin attestation; automation rejects, never accepts), and the census duty (every candidate flag recorded token-only). The ear-gate requirement gains the per-span attestation pin shape (dated user verdict → pin).

## Impact

- `frontend/src-tauri/src/audio/speaker/` — run-assembly promotion wiring, repair-window context/retry profile, census recording (token-only)
- `frontend/src-tauri/tests/` — red-first tests for promotion, retry profile, census record shape; ear-gate pin attestation via the existing fixture machinery (evidence home)
- GPU/live cost — offline probes + one production Speakers re-run with heartbeat; census pass runs first, reviewed before any render change
- Bright line unchanged: probes env-gated (`MEETIFY_LIVE_DIAG`), verbatim only under `MEETIFY_RENDER_PRINT`, clips ONLY in the evidence home or meeting folder, records token-only (counts, ratios, window coordinates, sha256)
- Prerequisite evidence: archived `2026-11-17-overlap-repair-net-worse-guard` (rebuild-notes.md, offline-evidence.md, issue #6) — the rebuild starts from its API sketches and kept window shapes
