## Context

The previous repair attempt (`overlap-repair-net-worse-guard`, archived 2026-11-17) proved on measurement that separated-stream decodes recover the true talk-over word order (the validated 32.69s window), then dropped the promotion: its automated acceptance bar — a per-badge token-conservation net-worse guard — could not separate a validated render from a paraphrased one (the mangled shape scored HIGHER; the noise of separation word-drop and cross-stream bleed dwarfs the 2–3-token fluent-substitution signal). The archived record concludes the paraphrase defect is meaning-level; the only oracle is the ear. The user's ruling (2026-11-18): decode reliability first, then ear pins.

What survives live and unchanged: the repair-window machinery (build/splice/absorption, floors), the separation adapter (probe-validated in-domain 2026-09-28), per-voice row rendering with margin-gated votes, hallucination guards, byte-identical degrade, `synth_parent` links, the census/replay/persistence-oracle harnesses, the ear-truth gate with the waiver router (pin-scoped, hard pins never waivable), and the conservation statistic as measurement instrumentation only. Rebuild material: the archived change's `rebuild-notes.md` (API sketches: `SpanDecodeEvidence`, `scramble_seed_spans`, `stream_decode_rate_fiction`, probe envs) and the three kept window shapes.

## Goals / Non-Goals

**Goals:**
- Separated-stream decodes inside repair windows recover verbatim text reliably, measured offline against the attested window shapes BEFORE any render ships.
- Every candidate overlap flag is recorded token-only (census) so detector precision is measured, not assumed.
- Qualifying talk-over spans promote through the live repair builder and render per-voice ONLY where the user has ear-attested the window (fail-closed, bright-line-safe).
- The user's dated per-span verdicts become permanent fixture pins the ear gate enforces.

**Non-Goals:**
- No new separation model or checkpoint work (the Conv-TasNet port passed its Phase-2 quality probe; swapping models is out of scope).
- No changes to the class-E floors or any detector dial — dial moves happen only on the user's dated rulings (standing ear-calibration hold).
- No summarizer, export, or UI changes. The attestation allowlist is a settings-level record, not a dialog.
- No attempt to resurrect an automated net-worse acceptance bar — that premise is falsified and recorded.

## Decisions

**D1 — Automation rejects, never accepts; acceptance = dated ear pins.**
The token-conservation guard is dead on measurement (archived record). The automated layer keeps only REJECTION duties it already has: rate-fiction/stutter stand-down, hallucination guards, margin-gated distinct-voice requirement, manual-row-wins, byte-identical degrade. Acceptance of a promoted render is the user's dated ear verdict, materialized as a fixture pin. Alternative rejected: bar R&D first (the user judged it a likely second dead end); decode-first attacks the measured failure rate instead.

**D2 — Attestation gates the render at runtime via a token-only allowlist, not fixture reads.**
Production code must never read the evidence fixture (bright line). So attestation ships as an app setting — `attestedRepairWindows`: a JSON list of `{start_s, end_s, attested_date}` entries the user maintains after each confirmation listen. The render splices per-voice rows ONLY for windows present in the allowlist AND passing every rejection guard; unpinned fired spans keep mixture rows and are recorded in the census as `awaiting-attestation`. The ear gate cross-checks (test side, which MAY read the fixture): every allowlist entry must match an attested fixture pin (walls + date), and every per-voice rendered span in the snapshot must have an allowlist entry — flipping the setting without pins fails the gate. Walls + date are token-only (coordinates), so the setting carries no meeting content.

**D3 — Decode-reliability levers, scoped to repair windows only.**
Stage 1 changes decode behavior INSIDE candidate repair windows only, never in the general transcription path:
- Wider window context: candidates rebuild at the evidence-backed ~9–12s context shape (decode quality grows with context; the span's own 1.3s walls garble).
- Retry profile: deterministic greedy/temperature-0 decode (already pinned for streams) plus a no-condition retry when the first pass trips rate-fiction or the stutter guards — the fluent-substitution mangle was a decode artifact, so the retry targets it where it lives.
- Measurement: probes run the three kept window shapes (validated 32.69s, mangled 30.68s / row-removed 30.0s, 12s control) against the attested fixture sequences read from the evidence home; a shape is "recovered" when the decode matches the attested sequence. STOP gate: if the retry profile cannot recover the attested shapes offline, the change reports and stops — the user re-plans before any wiring ships.

**D4 — Promotion rebuild from the archived sketches, through the live builder.**
Seed = a rate-fiction/stutter span that REACHED separation (duration-floor carve-out excluded). Its window rebuilds by the existing repair-window rules; two seeds in one window union; the promoted window routes through the live repair-window builder (separation + per-stream decode + per-voice splice with `synth_parent` links). Census records EVERY seed outcome token-only: `fired+attested`, `fired+awaiting-attestation` (mixture rows kept), `stood-down:<guard>`, `window-not-built`, `separation-unavailable`.

**D5 — Census before render changes.**
The env-gated candidate census (task 1) runs FIRST on the reference meeting: every candidate flag with overlap mass, duration, floors verdict — counts only. The user reviews it before any promotion wiring ships. It answers the detector-precision suspicion ("flagging talk-overs that do not exist") mechanically; ear rulings during the attestation round then measure precision on fired spans.

**D6 — Bright-line conventions inherited in full.**
Probes env-gated (`MEETIFY_LIVE_DIAG`, `#[ignore]`, loud skip without the evidence home); verbatim only under `MEETIFY_RENDER_PRINT`; records token-only (counts, ratios, window coordinates, sha256); clips written ONLY under the evidence home or the meeting folder; `cargo test -j 2`; no concurrent cargo during live runs.

## Risks / Trade-offs

- [Retry profile could alter decodes outside repair windows] → scoping is structural (repair-window-local), pinned by a byte-identity test on the general path.
- [Paraphrase class survives wider context/retry on some future span] → stage-1 STOP gate catches it on the attested shapes; at runtime the ear-pin flow rules it per-span (stand-down on a bad verdict).
- [Allowlist/pin mismatch (setting edited without a ruling)] → the ear gate cross-check fails loudly (waiver router refuses unpinned or stale-dated entries; no wildcard).
- [Pins are per-span; generalization unproven] → verdicts tune general dials only (standing rule); floors bound fired volume (~1–2/meeting), so attestation volume stays small.
- [Detector precision may measure poorly] → harmless while stands-down dominate (mixture rows keep correct text); the census informs the floor conversation but no dial moves without the user's ruling.
- [GPU/wall-clock] → census first (cheap, CPU), offline probes on three shapes, ONE production Speakers re-run with heartbeat; the replay/oracle harnesses verify mechanically after.
