## Why

The dropped `overlap-scramble-repair` change (fork issue #6) needed a runtime **net-worse guard** — a signal that a repair render degrades already-correct mixture text — before the scramble-seed promotion could ship again. This change set out to build that guard, settle its strictness offline first, and fix the ear gate's waiver absorption the same ear round surfaced. The offline measurement (task 1.4/1.5, 2026-11-17) settled the guard question negatively: NO conservation bar separates the user-validated render from the paraphrase-mangled one (the mangled case scores higher — a fluent substitution costs 2–3 tokens while separation word-loss costs more), so the repair path is closed by user ruling. What ships is the waiver-scoping fix.

## What Changes

- **CLOSED BY MEASUREMENT + USER RULING (2026-11-17): the scramble-seed promotion and the net-worse guard do not ship.** The offline evidence (recorded in `offline-evidence.md`, token-only) shows the conservation statistic cannot separate the validated render from the mangled one in either direction; the paraphrase defect is meaning-level, ruled only by the ear round (or the fixture pins — forbidden in code paths). The promotion stays dropped; issue #6 closes with the measured verdict. The measurement instruments (the pure statistic, the conservation scoping, the live probe) land as tested code because they are the instruments that produced the evidence.
- **Waiver-scoping fix (the surviving deliverable)**: a known-limitation record in the ear gate covers ONLY the assertions it documents — waivable pins gain labels, amendment records gain `covers` (pin labels and/or the gate's enumerated failure-kind ids), a failing assertion no record covers FAILS the gate instead of being silently absorbed. Hard pins are never coverable; no wildcard exists. The waiver resolution is extracted into ONE pure router shared by all four call sites, and the fixture-lint extends to both gate fixtures.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `speaker-diarization`: the ear-truth fixture gate requirement gains limitation-scoping (a known-limitation covers only the pins/checks it documents; hard pins are never waivable; the lint asserts the covers vocabulary on both fixtures).

## Impact

- **Code**: `frontend/src-tauri/tests/ear_truth_gate.rs` (pure waiver router + four call sites + lint); `RowPin.label` / `Amendment.covers` schema; `src/audio/speaker/net_worse_guard.rs` + `tests/net_worse_guard.rs` + `tests/guard_validation_probe.rs` (the measurement instruments, already landed and green — pure functions, std only).
- **Not shipped**: no promotion wiring, no production changes to the render path, no census cache. The render surface is untouched — the existing smoke specs' coverage is unchanged.
- **PII**: no meeting-derived content in the repo; the local evidence-home fixture gains pin labels + covers derived mechanically from its existing amendment records (task 4.3, evidence-home only).
- **Specs**: delta against `openspec/specs/speaker-diarization/spec.md` (requirement "Ear-truth fixture gate validates attribution").
