## 1. Code sweep (companion commit)

- [x] 1.1 Delete `use_cases/diarization_processor.rs` and its module declaration
- [x] 1.2 Remove queue plumbing: `JobPhase::Diarizing`, `diarization_processor` field/params, `with_all_processors`, dead chain branches, Group-8 tests
- [x] 1.3 `cargo test --lib` green (536 passed); zero references to the deleted symbols

## 2. Spec reconciliation

- [ ] 2.1 Delta files written for speaker-diarization (MODIFIED), post-meeting-pipeline (REMOVED ×2), recording-lifecycle (MODIFIED ×2)
- [ ] 2.2 `openspec validate` green; archive the change after the identity-stamping change lands (disjoint requirements, order not load-bearing)
