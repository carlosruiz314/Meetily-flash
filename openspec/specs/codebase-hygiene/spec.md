# codebase-hygiene Specification

## Purpose
TBD - created by archiving change decommission-screenpipe-stt-legacy. Update Purpose after archive.
## Requirements
### Requirement: The compiled crate declares all of its sources

Every `.rs` file under the Tauri crate's `src/` tree SHALL be reachable from a module
declaration: the build MUST NOT silently exclude source files, and dependencies that
are used only by excluded files SHALL NOT remain in `Cargo.toml`. A source file that is
no longer part of the build SHALL be deleted, not left orphaned on disk.

#### Scenario: Deleting orphaned files and unused dependencies keeps the build green

- **WHEN** undeclared source files (`src/audio/stt.rs`, `src/audio/core-old.rs`,
  `src/audio/system_audio_stream.rs`, `src/audio_v2/`) and the deps only they used
  (`crossbeam`, `dashmap`) are removed
- **THEN** `cargo build` and `cargo test --lib` stay green
- **AND** `cargo tree` no longer contains the removed dependencies
- **AND** no declared module references any deleted path

