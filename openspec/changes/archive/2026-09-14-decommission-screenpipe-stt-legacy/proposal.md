## Why

The screenpipe-derived transcription path is dead code in the strongest sense: it is not
even compiled. `src/audio/stt.rs` is declared in no module (`audio/mod.rs` has no
`mod stt;`), has zero callers, and imports modules that no longer exist on disk
(`crate::deepgram`, `crate::pyannote`, `crate::whisper` (candle), `crate::vad_engine`,
`crate::segments`, `screenpipe_core`) — it could not compile if it were included. It
predates the batch-only pivot and contradicts the no-live-recording-path architecture.

The same sweep of undeclared files found three more orphans, plus the Cargo dependencies
that only the dead path used:

- `src/audio/stt.rs` (screenpipe STT channel, candle/Deepgram/pyannote)
- `src/audio/core-old.rs` (pre-pipeline core)
- `src/audio/system_audio_stream.rs`
- `src/audio_v2/` (9 files, undeclared anywhere in `lib.rs`)
- `crossbeam` and `dashmap` crates: zero users in compiled code

No spec references any of these (`openspec/specs/` grep clean), so this is a pure
deletion — unlike `decommission-queue-diarization-phase`, no spec reconciliation is
needed.

## What Changes

- Delete the four undeclared/dead files/trees above and the `crossbeam` + `dashmap`
  entries from `frontend/src-tauri/Cargo.toml`.
- Update the stale `frontend/src-tauri/CLEANUP_PLAN.md` note that still instructs an
  `audio_v2` migration decision.
- No behavior change: none of it is part of the build. Compile time drops with the
  removed crates.

## Capabilities

### New Capabilities

- `codebase-hygiene`: sources under the crate must be declared and reachable from the
  build; deps used only by excluded files must go with them.

### Modified Capabilities

## Impact

- Code: deletions only, verified unreferenced. `cargo build` + `cargo test --lib` must
  stay green (they already exclude this code; the point is proving the deletion breaks
  nothing that exists).
