## Why

The overlap-scramble-repair change (applied, gate-run, ear-rounded, then DROPPED 2026-11-17 — fork issue #6) proved the scramble-seed promotion is a context lottery: the same builder produced a user-validated render at the ~1056s window AND a fluent paraphrase-mangle of attested text at the ~192s window, and no existing signal (stutter, rate ceiling, vote margin, separation collapse, word-loss count) catches fluent substitution of already-correct mixture text. The repair cannot ship until a runtime **net-worse guard** — a signal that a repair render degrades text that was already correct — exists. The same ear round also surfaced a gate defect: a known-limitation record silently absorbed a NEW failing text pin that its documented limitation never covered.

## What Changes

- **Rebuild the scramble-seed promotion from scratch** (panel-verified 2026-11-17: the dropped branch left NO commits — the applied work was uncommitted tree state discarded at the drop — so the source is issue #6's recorded findings plus the archived retranscription record): a stood-down overlap span whose separated decode exceeds the human speech ceiling (the interleave fingerprint) promotes its containing window to the separated-repair pipeline. Floors are untouched; the promotion is the only new fire path.
- **Net-worse guard (the core deliverable)**: before a promoted window's repair render replaces the mixture rows, the repair text OUTSIDE the seed span must conserve the mixture text it replaces — aligned PER BADGE (the repair render is two whole-window streams with no token timestamps; a global alignment caps near half even for a perfect repair), ordered, normalization-scoped, with a documented tolerance for the edge rows the window absorbs whole. Any violation stands the promotion down to the byte-identical mixture render. Principle: the repair may only change what is broken; text the promotion was not asked to fix must survive. Guard failures are logged in the census with the diverging evidence.
- **The guard's strictness is settled offline first**: hard interior conservation vs a ratio bar is decided by validating against the kept window-shape evidence (the validated 32.69s shape, the mangled 30.68s shape, the 12s control) BEFORE any production threshold exists — statistic frozen before numbers are recorded, a pinned separation margin between the cases, the 12s verdict pinned. Hard-conservation failure makes a user ruling MANDATORY (an implementer-chosen midpoint does not ship); the bar binds to ear-gate calibration governance, hold-outs excluded.
- **Gate lint fix**: known-limitation records gain pin-scoped coverage — a limitation documents WHICH pins it covers, and a pin failure outside every limitation's scope FAILS the gate instead of being absorbed.
- **Closure**: a production Speakers re-run plus a shrunken ear round — the fixture pins (validated-window render, attested-tail survival) are verified mechanically in the persisted rows first; the user's ear only judges newly-promoted regions the fixture does not already pin, if any exist.

## Capabilities

### New Capabilities

(none — the guard and promotion are rules of the existing run-assembly, not a new surface)

### Modified Capabilities

- `speaker-diarization`: the per-voice synthesis requirement is amended where the floors' unconditional byte-identity SHALL collides with seeding (a stood-down span may now seed a promoted window, whose own guard governs replacement); the ADDED scramble-seed/net-worse-guard requirement carries the promotion path, the conservation contract, promoted-row invariants, and the census/cache rules; the ear-truth fixture gate requirement gains limitation-scoping (a known-limitation covers only the pins/checks it documents; hard pins are never coverable).

## Impact

- **Code**: `frontend/src-tauri/src/audio/speaker/run_assembly.rs` (rebuilt seeding promotion + guard), the commands evidence pass, the census; `tests/ear_truth_gate.rs` + fixture lint (one pure waiver router, all four call sites); new env-gated probe for offline guard validation. Hexagonal: all are pure decisions over decoded evidence in the speaker use-case layer; the separation and transcriber ports are unchanged; no new adapters. The promotion enters as a third seed source in the post-duplicate-resolution repair fold, ranked stutter → loop → scramble.
- **Cost**: seed detection decodes the seed-decode set — trigger-fired spans that reached separation and render mixture-only (duration-floor carve-outs never separate and are excluded; partially-synthesized spans excluded) — a measured population, not assumed. The census pass runs FIRST and is reviewed before any production run; the per-span evidence cache becomes resumable (keyed by window geometry, not span walls) so the decode cost is paid once.
- **PII**: no meeting-derived content enters the repo. Guard validation runs env-gated against the local evidence home and recording folder; census/fixture references stay at the ID/timestamp/statistic altitude already committed in the archived specs.
- **Specs**: delta against `openspec/specs/speaker-diarization/spec.md` (requirements "Overlap spans render per-voice rows from separated streams", "Repair windows absorb the edge row whole", "Ear-truth fixture gate validates attribution").
