> **STATUS: DROPPED 2026-11-18 — never started (0/11 tasks, no code applied).**
> The motivating failure is already handled: batch transcription quarantines
> hallucinated segments (strict retry, then drop, with a mass-drop circuit
> breaker — landed 2026-09-13, fixture-verified; see the
> `post-meeting-pipeline` spec requirement). The change also predates the
> PII bright line: task 1.1's real recorded noise snippet as a repo fixture
> under `tests/fixtures/` is repo-forbidden now (would live in the evidence
> home). The knobs remain as this proposal found them
> (`RNNOISE_APPLY_ENABLED = false`, `positive_speech_threshold = 0.50`), so
> re-opening is cheap if noise-driven transcription failures ever reappear —
> re-scope to the current pipeline first.

## Why

After `fix-audio-normalization` and `post-meeting-transcription` are applied, the retranscription path may still forward borderline noise segments to Whisper if the ambient noise floor remains loud enough to pass the Silero VAD threshold (0.50 default) or if the MP4 audio still contains significant broadband noise. RNNoise is already compiled into the binary but disabled (`RNNOISE_APPLY_ENABLED = false`); enabling it in the recording path removes steady-state noise before it enters the MP4, improving both playback quality and retranscription accuracy. Silero threshold tuning tightens the VAD gate so only confident speech segments reach Whisper.

This proposal is intentionally held until results from the first two changes can be evaluated — it may prove unnecessary.

## What Changes

- Set `RNNOISE_APPLY_ENABLED = true` in `ffmpeg_mixer.rs` to enable RNNoise on the microphone channel before EBU R128 normalization
- Raise `positive_speech_threshold` in `vad.rs` from 0.50 to a tuned value (target: 0.60–0.70) based on empirical testing with ambient noise samples
- Add a unit test that asserts the VAD does not trigger on a recorded noise-only segment (keyboard clicks, HVAC) at the calibrated threshold

## Capabilities

### New Capabilities
- (none)

### Modified Capabilities
- `audio-recording-quality`: RNNoise suppression requirement changes from "intended but disabled" to "enabled by default"; Silero VAD threshold requirement added

## Impact

- `frontend/src-tauri/src/audio/ffmpeg_mixer.rs` — `RNNOISE_APPLY_ENABLED` constant
- `frontend/src-tauri/src/audio/vad.rs` — `positive_speech_threshold` value
- Applies to both the recording path (MP4 quality) and the retranscription VAD path
- **Prerequisite**: `fix-audio-normalization` and `post-meeting-transcription` should be applied and tested first; this change addresses residual issues only
