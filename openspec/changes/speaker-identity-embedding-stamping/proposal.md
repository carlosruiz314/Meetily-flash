## Why

Every diarization run persists speaker centroid embeddings with `speaker_id = NULL` (`run_diarization_for_meeting`, `commands.rs:518` — the only live store path; the queue-wired `DiarizationProcessor` is production-dead). The cross-meeting matcher is an in-memory registry hydrated once at startup from `list_all_embeddings`, keyed by `COALESCE(speakers.name, cluster_label)` — a name string. Result: meeting A's auto row "Speaker 0" and meeting B's auto row "Speaker 0" pool into one identity, renames after startup are invisible until relaunch, and rename-based identification can never carry across meetings.

## What Changes

- The live store path (`run_diarization_for_meeting`) persists every centroid embedding stamped with a concrete `speakers.id`: clusters matched to a named speaker link to it; unmatched clusters link to a meeting-local auto row (`speaker-auto-{meeting_id}-*`). Delete + insert of a meeting's embeddings becomes one transaction; store errors propagate instead of being logged and swallowed.
- The matcher pool becomes stamped embeddings of **named speakers only**, keyed by speaker id (not name), reloaded from the DB at the start of every Speakers run instead of the startup-frozen snapshot. Auto rows never anchor cross-meeting identity. The hard-coded 0.60 registry threshold is replaced by the configured threshold (default 0.40, [0.35, 0.70]).
- `label_speaker` performs the embedding linking the spec already mandates (today it touches only transcript labels): relink by speaker id for renamed/re-matched clusters; `revert_speaker_label` unlinks symmetrically, replacing the corrupt `cluster_label NOT IN (...)` SQL.
- **BREAKING** (data semantics): a migration sweeps `speaker_embeddings WHERE speaker_id IS NULL`. NULL rows remain a legitimate steady-state value for deliberately unlinked embeddings (revert, speaker deletion) — the matcher ignores them permanently, so no further sweeps are needed. Regeneration of legacy identity data = re-running Speakers per meeting.
- No change to transcript labels, clustering, the Speakers-button flow, or rename UI mechanics.

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: two requirement-level changes — (1) new requirement for identity-stamped persistence (non-null ids, transactional replacement, auto-row lifecycle); (2) MODIFIED "Cross-meeting speaker matching uses embedding similarity" — pool composition, keying, refresh timing, and threshold source all change.

## Impact

- `frontend/src-tauri/src/audio/speaker/commands.rs` — live store path stamping, transactional replace, `label_speaker`/`revert_speaker_label` linking, threshold from settings, prune auto rows on meeting deletion
- `frontend/src-tauri/src/database/setup.rs` — registry hydration keyed by speaker id; refresh hook reused per run
- `frontend/src-tauri/src/database/repositories/speaker.rs` — stamped-pool query, relink/unlink by id, transactional store
- `frontend/src-tauri/migrations/` — NULL sweep migration
- `frontend/src-tauri/src/use_cases/diarization_processor.rs` — untouched (production-dead; doc comment updated to say so)
