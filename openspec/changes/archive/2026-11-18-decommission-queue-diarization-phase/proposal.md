## Why

The transcription queue's `Diarizing` phase never executes in production: the queue is wired with `diarization_processor: None` everywhere, and the live diarization path is `run_diarization_for_meeting` (invoked by import, Enhance/retranscription, and the Speakers/reset commands). The dead transport (`use_cases/diarization_processor.rs`, the `JobPhase::Diarizing` variant, queue chaining) was swept from the code in this change's companion commit, but four spec requirements across three capabilities still mandate the dead queue-phase behavior. This change reconciles the specs with the shipped reality.

## What Changes

- Code (companion commit, same branch): delete `use_cases/diarization_processor.rs` and the queue's diarization plumbing — `JobPhase::Diarizing`, the `diarization_processor` field/parameter, `with_all_processors`, the dead chain branches, and the Group-8 tests pinning them. No live behavior changes.
- `speaker-diarization`: MODIFIED headline requirement — diarization transport re-pointed from "queue phase" to `run_diarization_for_meeting`; dead queue-chaining scenarios replaced with the live invocation scenarios. The pipeline substance (steps 1–7, pyannote amendment prose) is unchanged.
- `post-meeting-pipeline`: REMOVED "Diarizing phase chains after Summarising in the queue" and "Diarizing processor decodes audio and runs offline diarization" — both describe the deleted machinery.
- `recording-lifecycle`: MODIFIED two requirements — same 1-second status-bar guarantee, now stated against on-demand diarization; `TranscriptSegment.speaker` populated by the diarization service (event payload corrected to the shipped `{meeting_id, speaker_count, segments_labeled}`).

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: headline requirement's transport clause and queue scenarios re-pointed to the live path
- `post-meeting-pipeline`: two queue-diarization requirements removed
- `recording-lifecycle`: two requirements re-pointed from the queue phase to the diarization service

## Impact

- Code: `frontend/src-tauri/src/use_cases/diarization_processor.rs` (deleted), `use_cases/mod.rs`, `use_cases/transcription_queue.rs` (plumbing removed; 536 lib tests green)
- Specs only otherwise — no runtime behavior change beyond the removal of unreachable code
