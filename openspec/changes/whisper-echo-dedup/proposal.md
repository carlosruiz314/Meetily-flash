# Proposal: whisper-echo-dedup

## Why

whisper occasionally emits the same sentence twice in one row — the second copy
with degenerate token walls (all pieces pinned to a single zero-span point,
usually the row's end edge). Measured case (cde5c264, source row 1193.55–1215.61):
"I am okay to do it as long as they give us an actual plan." appears twice; the
first copy has real walls (1200.40–1208.50s), the second is collapsed to a point
at 1215.60. The render splits the row at the voice boundary, so the ghost copy
became the opening of UserB's row — reading as if Speaker 2 said the sentence
and UserB immediately repeated it verbatim. User-visible fabrication.

## What Changes

- One pure filter in the token path: within a row's token stream, a run of
  pieces whose normalized text repeats the immediately preceding run of equal
  length AND whose walls are acoustically weightless (all zero-span, or all
  inside a <50 ms window) is dropped — the ghost copy. Repetitions with real
  walls (people genuinely repeating: "No, no, no", a real echo) are kept.
- The ghost's text disappears from the render; the copy with real walls stays.

## Impact

- Affected: `alignment.rs` (one filter + tests), gate fixture unchanged (it
  pins source rows, not render output). Live re-persist removes the ghost from
  the 1211.14 UserB row.
- Out of scope: the S16 overlap scramble (needs separation + re-decode;
  overlap-separation-prepass), any re-transcription.
