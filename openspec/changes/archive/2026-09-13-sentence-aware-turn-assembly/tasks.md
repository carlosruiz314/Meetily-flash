## 1. Pure assembly core

- [x] 1.1 RED→GREEN: `detokenize` — contractions reattached, no space before sentence punctuation, whitespace collapsed ("How 's it going ?" → "How's it going?")
- [x] 1.2 RED→GREEN: `assemble_turns` — same-speaker merge ≤3 s gap, speaker flip never merges, punctuation-only rows dropped, time span = first.start→last.end
- [x] 1.3 Property: assembly never loses alphanumeric content (concatenation of inputs ⊆ concatenation of outputs, modulo whitespace/punctuation spacing)

## 2. Persist path + consolidation

- [x] 2.1 Wire `assemble_turns` into `run_diarization_for_meeting` post-alignment, pre-persist
- [x] 2.2 RED→GREEN: `consolidate_meeting_turns` repo fn — transactional insert+delete; idempotent second run; failure leaves rows untouched

## 3. Live data + verification

- [x] 3.1 Backup live DB, run consolidation on cde5c264, verify: fragment ratio (<10% rows not ending in sentence punctuation), no punct-only rows, spacing fixed, speakers/counts sane
- [x] 3.2 `cargo test --lib` green
- [x] 3.3 OpenSpec archive — DONE 2026-09-13 (delta synced to `openspec/specs/speaker-diarization/spec.md`; folder moved to `archive/2026-09-13-sentence-aware-turn-assembly`)
