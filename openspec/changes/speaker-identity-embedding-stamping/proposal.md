## Why

Every diarization run persists speaker centroids to `speaker_embeddings` with `speaker_id = NULL` (hard-coded in `diarization_processor.rs`), so the cross-meeting matcher pools vectors by per-meeting cluster label — "Speaker 0" of one meeting is pooled with "Speaker 0" of every other meeting. Identity matching is therefore meaningless today, and rename-based identification ("host is Cynthia") can never carry across meetings.

## What Changes

- Diarization persists each centroid embedding stamped with a real `speakers.id`: after cross-meeting matching, matched clusters link to the matched speaker; unmatched clusters create a new `speakers` row and link to it. The unused `link_embedding_to_speaker` repository method becomes the (wired-in) linking step.
- The cross-meeting matcher consumes only stamped embeddings (`speaker_id IS NOT NULL`); NULL rows are ignored, not pooled by cluster label.
- **BREAKING** (data semantics, not API): previously stored NULL-speaker_id embeddings are dead weight — a one-time sweep delete of `speaker_embeddings WHERE speaker_id IS NULL` runs as part of rollout, and regeneration happens by re-running Speakers per meeting (the run already deletes and rewrites that meeting's rows).
- No change to transcript labels, the Speakers button flow, or the rename UI; names written through the rename UI become the identity that future runs match against.

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: new requirement covering embedding persistence — centroids must be stamped with a concrete speaker id at write time, the matcher must ignore unstamped rows, and re-running Speakers on a meeting replaces that meeting's embeddings atomically.

## Impact

- `frontend/src-tauri/src/use_cases/diarization_processor.rs` (store path: stamp after `match_speakers`; today it passes `None` hard-coded)
- `frontend/src-tauri/src/database/repositories/speaker.rs` (`link_embedding_to_speaker` wiring; NULL-ignoring variant of `list_all_embeddings` for the matcher)
- `frontend/src-tauri/src/audio/speaker/commands.rs` (per-meeting delete already exists — unchanged)
- `speakers` table gains auto-created rows for unmatched clusters (naming them stays a UI/rename action)
- One-time SQL sweep for legacy NULL rows; user-facing regeneration = clicking Speakers per meeting
