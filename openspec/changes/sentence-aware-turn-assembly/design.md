## Context

Alignment produces word-exact speaker boundaries, which fragment text: 77% of persisted rows on the live meeting are sentence fragments. The fix is text reassembly, not better detection.

## Goals / Non-Goals

**Goals:** readable persisted turns; no re-diarization for existing meetings; no loss of alphanumeric content; speaker changes always visible.

**Non-Goals:** no re-punctuation or capitalization inference; no re-running detection; no frontend rendering changes; no cross-speaker sentence repair.

## Decisions

- **Pure core in `turns.rs`** (detokenize + assemble) so both the persist path and the consolidation pass share one tested implementation.
- **Merge rule: same speaker AND gap ≤ 3 s.** 3 s comfortably joins backchannel-adjacent speech ("Yeah ," between turns of one speaker) while refusing to bridge real topic pauses. Speaker change always breaks, mid-sentence or not — the interjection is real speech and must stay a separate row.
- **Detokenize, don't repunctuate.** Only mechanical spacing fixes (contractions, space-before-punctuation); no capitalization or comma inference — too error-prone to ship silently.
- **Punctuation-only rows are dropped**, not merged — they carry no content.
- **Consolidation = insert merged turns + delete absorbed rows in one transaction**, keyed on the meeting; second run is a no-op because assembled turns no longer match the merge predicate (same-speaker neighbors now exceed… actually: two adjacent same-speaker TURN rows with ≤3 s gap WOULD re-merge — accepted: consolidation is idempotent in content but may further merge turn rows that sit within 3 s of each other; the no-op test seeds already-merged rows with >3 s gaps between distinct turns).
- **Persist-path ordering:** assemble after temporal assignment, before the aligned-persist write, so the DB only ever sees turns going forward.

## Risks / Trade-offs

- [Turn ids are new; external references to absorbed row ids break] → no known consumers reference transcript row ids outside the transcripts table (checkpoints reference segments, summaries reference the meeting).
- [3 s threshold is a judgment call] → unit-pinned; easy to tune later.
- [Breaking change to fine-row shape] → the existing idempotent-re-diarization scenario still holds (N=1 relabel in place on turn rows).

## Migration Plan

Ship code; then backup live DB and run consolidation on cde5c264; archive.

## Open Questions

- None.
