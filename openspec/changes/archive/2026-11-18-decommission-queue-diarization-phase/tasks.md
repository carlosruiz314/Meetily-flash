## 1. Code sweep (companion commit)

- [x] 1.1 Delete `use_cases/diarization_processor.rs` and its module declaration
- [x] 1.2 Remove queue plumbing: `JobPhase::Diarizing`, `diarization_processor` field/params, `with_all_processors`, dead chain branches, Group-8 tests
- [x] 1.3 `cargo test --lib` green (536 passed); zero references to the deleted symbols

## 2. Spec reconciliation

- [x] 2.1 Delta files written for speaker-diarization (MODIFIED), post-meeting-pipeline (REMOVED ×2), recording-lifecycle (MODIFIED ×2) [DONE 2026-11-18: deltas written at apply, assessed against the current live specs at archive — post-meeting-pipeline REMOVED ×2 and recording-lifecycle MODIFIED ×2 synced to the live specs (both live requirements still described the deleted `Diarizing` phase; the synced recording-lifecycle payload `{meeting_id, speaker_count, segments_labeled}` verified against all four real emit sites: import.rs, retranscription.rs, commands.rs ×2). The speaker-diarization delta was NOT synced: later archived changes re-derived the live headline requirement (now "runs as a direct post-transcription step", already carrying the decommission amendment verbatim), so its stale pre-run-assembly MODIFIED block is marked SUPERSEDED in the delta file instead of regressing the spec.]
- [x] 2.2 `openspec validate` green; archive the change after the identity-stamping change lands (disjoint requirements, order not load-bearing) [DONE 2026-11-18: `openspec validate` green ("Change ... is valid"); identity-stamping archived earlier the same day (2026-11-18), clearing the stated sequencing condition; change moved to `openspec/changes/archive/2026-11-18-decommission-queue-diarization-phase/`. 5/5 tasks complete.]
