## 1. Pre-deletion proof

- [x] 1.1 Re-verified zero references: no `mod`/`use`/`include!`/`#[path]` references to `stt`, `core-old`, `system_audio_stream`, `audio_v2` in compiled sources; `crossbeam`/`dashmap` used only inside the undeclared `stt.rs`
- [x] 1.2 Baseline: `cargo test --lib` green before deletion (648 passed, 0 failed)

## 2. Deletion

- [x] 2.1 Deleted `frontend/src-tauri/src/audio/stt.rs`, `frontend/src-tauri/src/audio/core-old.rs`, `frontend/src-tauri/src/audio/system_audio_stream.rs`
- [x] 2.2 Deleted `frontend/src-tauri/src/audio_v2/`
- [x] 2.3 Removed `crossbeam` and `dashmap` from `frontend/src-tauri/Cargo.toml` (lock refreshes on next build); staging/commit is the principal's act
- [x] 2.4 `cargo test --lib` green post-deletion (666 passed — includes the whisper-hallucination-cleanup tests added in the same working session); `dashmap` is fully out of the graph; remaining `crossbeam-utils/-channel/-epoch/-deque/-queue` entries are TRANSITIVE sub-crates of other packages, not the removed direct dep (task wording corrected accordingly)

## 3. Docs and validation

- [x] 3.1 Deleted `frontend/src-tauri/CLEANUP_PLAN.md` (still instructed an "audio_v2 migration or remove" decision — superseded by this change)
- [x] 3.2 `openspec validate decommission-screenpipe-stt-legacy` green
