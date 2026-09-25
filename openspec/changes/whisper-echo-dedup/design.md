# Design: whisper-echo-dedup

## Mechanism (one pure filter)

`dedupe_degenerate_repeats(pieces: Vec<TokenWord>) -> Vec<TokenWord>` in
`token_timestamps.rs` (next to `is_eot_marker`), applied inside
`valid_token_words` before the piece→word merge.

Detection per index i: let run R = the maximal normalized-equal rerun of the
preceding pieces (compare normalized concatenation — lowercase, strip
non-alphanumerics — of the k pieces ending at i−1 against the k pieces starting
at i, k ≤ 32). R qualifies as a ghost when every piece in it is acoustically
weightless: `end_ms − start_ms ≤ 1` for all pieces, or `max(end) − min(start) <
50` ms. Ghost pieces are skipped. The first copy (real walls) is untouched.

Honest-evidence guards:
- Only equality + weightlessness drops. Real repeats have real walls → kept.
- Both copies weightless → the whole region is degenerate; drop the second
  copy only (the first may still be resolvable by the wall-merge clamp).
- A repeat spanning a sentence boundary is still eligible (the measured ghost
  is a full sentence).

## Adversarial tests (RED before GREEN)

1. Real fixture shape (trimmed from cde5c264 1193.55 row): first copy real
   walls, second copy zero-span pinned → second copy dropped, first intact.
2. "No, no, no" with real walls → kept.
3. Real repetition, both copies real walls → kept.
4. Both copies weightless → second dropped, first kept.
5. Property: output piece count never exceeds input; surviving pieces keep
   original walls and order.

## Addendum (2026-09-25, measured on the real 1193.55 row)

The motivating row never reached the word-level dedup: its piece stream
diverges from its text (three audibly-real `cool` pieces, text writes two), so
the strict merge rejects the WHOLE row to proportional walls — where the ghost
sentence has no weightless signature and renders across the badge boundary.
Two composed repairs, both evidence-gated:

1. **Bounded backtrack merge**: when a word's accumulator overshoots
   (`len(acc) >= len(want)` with `acc != want`), retry the word with one
   already-consumed piece removed — allowed only when the removed piece is
   weightless OR its normalized text duplicates a piece within 2 stream
   positions (the measured case: `cool` at −2). The word must then merge
   EXACTLY (text spelling is never relaxed). Caps: ≤2 backtracks per word,
   ≤ max(pieces/8, 2) per row (the floor lets tiny rows use the repair at all); beyond caps → honest proportional fallback.
2. **Word-level ghost dedup** (as designed): after a successful merge, the
   weightless copy of a repeated sentence drops (the 1215.6-point tail).

Fallback order per row: strict merge → backtrack merge → proportional. The
ghost text is never "fixed" by editing text — only pieces the stream itself
marks as duplicate/weightless are skipped, and only whole-sentence repeats
whose second copy is weightless are dropped at word level.
