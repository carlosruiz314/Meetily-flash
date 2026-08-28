## REMOVED Requirements

### Requirement: Diarizing phase chains after Summarising in the queue

**Reason**: The queue's `Diarizing` phase never executes — the queue is wired with `diarization_processor: None` everywhere, and diarization runs via `run_diarization_for_meeting` (import, Enhance/retranscription, Speakers commands). The `JobPhase::Diarizing` variant and chaining branches were deleted; the queue is now two-phase (`Transcribing`, `Summarising`).

**Migration**: None needed — the chained phase never fired in production. Cross-meeting and post-transcription diarization behavior is specified by the `speaker-diarization` capability's headline requirement (transport re-pointed to `run_diarization_for_meeting`).

### Requirement: Diarizing processor decodes audio and runs offline diarization

**Reason**: The `diarization_processor` function this requirement mandates was production-dead (`DiarizationProcessor::process` was never constructed into the queue) and has been deleted. The live pipeline performs the same steps via `run_diarization_for_meeting`, specified by the `speaker-diarization` capability.

**Migration**: The pipeline substance (decode → pyannote-boundary chunks → embeddings → cached clustering → short-speaker merge → alignment → persistence) is specified by the `speaker-diarization` capability's headline requirement. The `diarization-complete` event continues to be emitted by the live paths with payload `{meeting_id, speaker_count, segments_labeled}`.
