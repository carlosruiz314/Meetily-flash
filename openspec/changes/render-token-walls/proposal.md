## Why

The S16 talk-over fix (`talkover-word-order-repair`, paused at §2 on the
user's 2026-11-18 ruling) is measured blocked by render geometry: the
region renders as one 32s row [1042.40,1074.42] (created 2026-09-14)
spanning two source chunks, and the repair splice's absorb-whole rule
cannot split it safely — mixture render rows carry no per-word walls, so
a partial split would misplace or duplicate words (both admissible wall
strategies measured and failed: `probe-record-2-4.md`,
`mega-row-investigation.md`). The sources DO carry token timestamps and
the alignment pipeline already computes real per-word walls (`UnitWord`s)
internally — they are simply dropped at persist (`token_timestamps =
NULL`, pinned "expected and permanent" in the live spec). The user ruled
the token-walls change runs FIRST as the unblock path.

## What Changes

- The alignment pipeline persists the per-word walls it already computes:
  every aligned segment carries its `UnitWord` walls through to the
  rendering row's existing `token_timestamps` column (NULL today).
- Consolidation merges concatenate the merged rows' token walls (walls
  preserved verbatim, never re-normalized).
- Rendering rows of token-less legacy sources persist NULL walls and the
  existing proportional fallback is untouched.
- Split-faithfulness is pinned: word multiset AND wall integrity
  (each row's token walls sit inside the row's own walls, monotone
  non-decreasing) extend the existing word-survival guarantee.
- No new consumer ships in this change: the paused
  `talkover-word-order-repair` change resumes its §3 attested-route
  wiring on top (its straddle policy splits a token-walled straddling
  row at real token boundaries — the S16 unblock).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `speaker-diarization`: the "Token-level timestamps align transcript
  text with diarization speaker boundaries" requirement's persist
  semantics — rendering rows carry aligned token walls (the
  "token_timestamps = NULL is expected and permanent" clause is
  superseded); consolidation preserves walls; the word-survival
  guarantee extends to walls.
