# Tasks: whisper-echo-dedup

- [x] 1.1 RED: `dedupe_degenerate_repeats` unit tests (5 cases above).
- [x] 1.2 GREEN: filter + wire into `valid_token_words` pre-merge; lib tests.
- [x] 1.3 Gate: 18/18 (S7c hard pin), 0 fractures; live re-persist; the
      1211.14 UserB row must start "and what they want…" and Speaker 2's
      row keeps one copy of the sentence.

## Completion note (2026-09-25)

The motivating row needed MORE than word-level dedup: its piece stream had
three audibly-real `cool` pieces for two text words, so the strict merge
rejected the whole row to proportional walls (where the ghost had no weightless
signature). Added the bounded backtrack merge (design addendum): overshoot
retries drop a consumed piece only when it is weightless or duplicates a piece
within 2 stream positions (raw or normalized equality); caps 2/word, pieces/8
per row; a word's only piece is never droppable. Live result: the 1193.55 row
merges with real walls, Speaker 2's row ends '...it's more than fine.' and
UserB's row opens with ONE 'I am okay to do it as long as they give us an
actual plan.' — the fabricated verbatim echo across the badge boundary is gone
(271 -> 269 rows). Gate 18 pass + 1 amended (S16) + 0 FAILED; 308 lib tests.

## §3 smoke-spec decision

No E2E smoke spec: backend-only text repair inside the diarization render; the
outcome is pinned by the offline gate replay over the fixture (the echo row is
in the pinned snapshot) + persisted-row dumps — same rationale as
turn-boundary-wall-realignment.
