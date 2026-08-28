## MODIFIED Requirements

### Requirement: Diarization queue phase does not delay the recording-stop status bar guarantee

Diarization SHALL NOT block or delay the existing 1-second status-bar-clear guarantee of `stop_recording`: the `RecordingStatusBar` disappears within 1 second of stream release regardless of whether diarization is enabled or still pending. Diarization is not a queue phase and never runs inside the recording-stop path; it runs later via `run_diarization_for_meeting` (import, Enhance/retranscription, or the Speakers commands).

#### Scenario: Diarization does not delay the 1-second status bar clear

- **GIVEN** a recording is active and speaker diarization is enabled
- **WHEN** `stop_recording` is invoked
- **THEN** the status bar still clears within 1 second of stream release
- **AND** diarization, whenever it later runs via `run_diarization_for_meeting`, never blocks the status bar UI

### Requirement: TranscriptSegment carries an optional speaker field

`TranscriptSegment` SHALL include an optional `speaker: Option<String>` field. The field SHALL be `None` until diarization has run for the meeting via `run_diarization_for_meeting` (no speaker labels are available before diarization runs), and SHALL be populated on the transcript rows after diarization. Speaker labels are not delivered via any realtime event during recording; transcription and diarization both run post-meeting (see `audio-recording-quality`).

#### Scenario: Transcript speaker populated after diarization

- **WHEN** diarization completes for a meeting via `run_diarization_for_meeting`
- **THEN** the transcript rows in the database have `speaker` set to the assigned label
- **AND** the frontend receives a `diarization-complete` event with `{meeting_id, speaker_count, segments_labeled}`
