## Why

The ear-truth render replay over meeting cde5c264 — production align → borrow → merge → consolidate over real DB rows ([evidence](../../exploration/render-print-20260908-b.log)) — writes 240 persisted rows, 163 of which begin mid-sentence, the user's immediate-suspect screen. Engine turn boundaries pass the ear pins (15 PASS, 1 known limitation, 0 turn-level violations), but row TEXT is divided at those boundaries and the halves land under different badges: "Yeah, for Paulina, right? Where is" (Speaker 1) / "Ricardo" (Speaker 0) / "I" (Speaker 1) / "don't know. Let me ping in..." (Speaker 0) — "Where is Ricardo?" and "I don't know" each fractured across rows with the tail under the wrong badge. An adversarial panel round (2026-09-08, 6 reviewers) further established: the duplicate instance is a 3-row chunk-overlap re-transcription cluster ([74.28]/[80.34]/[85.49], not an equal-text pair), 237/240 rows have NULL token timestamps (proportional spans are the real regime), and the live DB mutates under the gate (a Speakers run at 2026-09-08 13:39 relabeled all rows; Enhance auto-runs diarization).

## What Changes

- **Sentence-atom assignment**: transcript text is segmented into sentences (proportional per-sentence spans by default — the 237/240 NULL-token regime — token spans only when a sanity clamp passes) and each sentence is assigned WHOLE to the engine turn owning the majority of its span. No sentence's text is ever divided across two badges; a straddling sentence yields two atoms with the tail flagged, never a cross-badge fragment. Minority-span words move with their sentence (reattribution — engine turn boundaries unchanged).
- **Duplicate clusters merged, not dropped**: adjacent-in-time rows sharing a ≥3-token contiguous cross-badge match (≥80% of the shorter row, disjoint spans, ≤2 s gap) resolve to one row — text written once, span extended to the union, absorbed shells AND their source rows deleted in the persist transaction (no unlabeled orphans).
- **Gate hardening over a snapshot-pinned replay**: the input rows are snapshotted into the fixture with a hash (the live DB mutates under every run — Enhance included); the ear gate asserts 0 cross-badge fractures (predicate: mid-sentence start + different badge + previous row lacks terminal punctuation — NOT raw lowercase, which is unsatisfiable on ASR text) and 0 duplicate clusters, with a user-signed amendment record as the only waiver path.
- **Spec reconciliation**: full MODIFIED blocks bring the canonical token-alignment and run-assembly requirements to sentence granularity (they currently mandate split-at-turn-boundary text division — the defect); the canonical queue-phase postscript ("token-less rows split proportionally at turn boundaries") is superseded by the sentence-granularity mandate (archive-gate checked).
- **Sequencing**: gap-speech 4.2 replay diff lands before this change's asserted gate runs; ONE combined user-gated live run verifies gap-speech 5.2 and this change together; archive follows sentence-aware-turn-assembly.

## Capabilities

### New Capabilities

### Modified Capabilities

- `speaker-diarization`: new requirement — sentences are not split across speaker badges (assignment atom, reattribution, duplicate clusters, fracture gate); MODIFIED — "Token-level timestamps align transcript text with diarization speaker boundaries" and "Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits" move text division from turn-boundary word/proportional splits to sentence-granularity whole-sentence assignment.

## Impact

- `frontend/src-tauri/src/audio/speaker/alignment.rs` — sentence segmentation + sentence-atom assignment
- `frontend/src-tauri/src/audio/speaker/commands.rs` — duplicate resolution in the persist path; `stamp_continuation_facts` overlap-match fix
- `frontend/src-tauri/src/database/repositories/speaker.rs` — duplicate shell/source deletion inside the persist transaction
- `frontend/src-tauri/tests/ear_truth_gate.rs` — snapshot-pinned replay, fracture + duplicate assertions, waiver path
- `frontend/src-tauri/tests/fixtures/cde5c264_transcripts.json` — new snapshot fixture
- Existing alignment split-pinning tests rewritten to sentence-atom expectations
- Live verification: one combined Speakers run on meeting cde5c264 after implementation (user-gated, shared with gap-speech-voice-attribution 5.2)
