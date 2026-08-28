## Why

On the live 83-minute meeting, 331 of 430 speaker-labeled transcript rows (77%) are sentence fragments: alignment splits rows at every speaker flip — including sub-second backchannels ("Yeah ,") — and nothing ever re-assembles same-speaker neighbors into readable turns. Token-joined spacing ("that 's", "go .") and punctuation-only rows make it worse. The user rejected the transcript as unreadable: "cuts off mid-sentence … chaotic." The diarization boundary signal itself is good; the text-reassembly layer it feeds is missing.

## What Changes

- New pure `turns` module: detokenizes whisper word-joined text (contractions, space-before-punctuation), drops punctuation-only rows, and merges adjacent same-speaker rows (gap ≤ 3 s) into sentence-bounded speaker turns. Speaker changes always start a new turn.
- The diarization persist path writes assembled turns instead of raw aligned fragments.
- A transactional consolidation pass applies the same assembly to already-persisted meetings (no audio, no re-diarization); run once on the live meeting.
- **BREAKING** (data shape): adjacent same-speaker rows no longer persist as separate fine rows. Multi-speaker splits are unaffected — different speakers never merge.

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: new requirement for speaker-turn assembly (readability contract); existing split-persistence scenarios remain true (multi-speaker splits still persist as separate rows) and are extended with the same-speaker consolidation rule.

## Impact

- `frontend/src-tauri/src/audio/speaker/turns.rs` (new, pure, unit-tested)
- `frontend/src-tauri/src/audio/speaker/commands.rs` (persist path assembles turns before writing)
- `frontend/src-tauri/src/database/repositories/speaker.rs` (transactional consolidation for existing meetings)
- Live data: one backup + consolidation run on meeting cde5c264
