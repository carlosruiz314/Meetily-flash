## Context

The paused `talkover-word-order-repair` change measured its S16 fix
blocked on render geometry (`probe-record-2-4.md` +
`mega-row-investigation.md`): the S16 talk-over sits inside one 32s
render row [1042.40,1074.42] that spans two source chunks and carries no
per-word walls, so the repair splice's absorb-whole rule cannot split it
safely (mixture rows are atomic text blobs — a partial split would
misplace or duplicate words). The sources carry token timestamps and the
alignment pipeline already computes real per-word walls internally
(`valid_token_words` → `UnitWord`s — the wall-vote machinery of the
September work); they are dropped at persist (`token_timestamps = NULL`,
pinned "expected and permanent" in the live spec). The user ruled
(2026-11-18) that this change runs FIRST as the unblock path.

The original deferred token-walls work (fork issue #4) targeted SYNTH
rows — walls from SEPARATED-stream decodes, with a measured DTW-smear
risk requiring an ear round. THIS change is a different consumer with
cleaner inputs: walls from SOURCE decodes (mixture chunks — the
wall-vote machinery already trusts them) carried into PLAIN render rows.
No separated-stream walls are in scope; no new ear round gates the
mechanism (the rendered outcome is text-identical; the paused change's
ear pins cover it when it resumes).

## Goals / Non-Goals

**Goals:**
- Rendering rows persist the aligned per-word walls the pipeline already
  computes (existing `token_timestamps` column) — the render becomes
  token-wall-mapped whenever its source is.
- Split and consolidation stay wall-faithful: pieces carry exactly their
  own words' walls; merges concatenate verbatim.
- Text output is byte-identical to today's render for unchanged input —
  only the token column changes.

**Non-Goals:**
- No consumer wiring: the paused change's attested-route straddle policy
  resumes on top; no engine behavior reads the persisted walls in this
  change.
- No synth-row walls (the repair splice keeps NULL — the paused change
  owns that surface; separated-stream walls stay deferred with issue #4's
  ear-round gate).
- No schema change (the column exists on both tables), no dial moves, no
  UI changes.

## Decisions

**D1 — One-way wall flow: source → alignment → render.** The `UnitWord`
walls the alignment already derives from source token timestamps are the
single wall truth. Every aligned segment carries the walls of exactly the
words its text holds (punctuation-only ranges dropped, consistent with
the existing atom rules); nothing re-derives or re-stamps walls anywhere
downstream. Consolidation concatenates the merged rows' lists verbatim in
row order. The aligner still never reads the rendering rows (the
immutable-source doctrine is unchanged — walls ride the same path as
text).

**D2 — Persist: the existing column, the existing shape.** Regeneration's
INSERT writes the segment's wall list (the `TokenWord` JSON shape the
source carries) into `token_timestamps`; token-less segments persist NULL
and the legacy proportional fallback is byte-identical. Repair-spliced
synth rows keep NULL (owned by the paused change). Manual rows survive
untouched as today.

**D2 — Persist: the existing column, the existing shape.** Regeneration's
INSERT writes the segment's wall list (the `TokenWord` JSON shape the
source carries) into `token_timestamps`; token-less segments persist NULL
and the legacy proportional fallback is byte-identical. Repair-spliced
synth rows keep NULL (owned by the paused change). Manual rows survive
untouched as today. Writer inventory (explore pass, 2026-11-19): the
render table has exactly two production persist paths —
`insert_transcription_row` (Enhance/retranscribe; ALREADY wall-bearing
from the source) and `persist_regenerated_rendering` (Speakers
regeneration; the NULL-walled site this change modifies). The generation
path is what erases walls today; no other site needs touching.

**D3 — Wall integrity as a pinned invariant.** Every walled row: each
token wall within the row's own bounds, start times monotone
non-decreasing, and the list's alphanumeric words equal the row text's
alphanumeric words in order. The word-survival guarantee extends to
walls (per-row form strengthened where the source is walled). For
validator-accepted walls this invariant is a CONSTRUCTION POSTCONDITION,
not a new stricter semantics: `valid_token_words` matches pieces to text
words one-for-one (skipped pieces are extras the text does not spell),
so each formed segment's walls equal exactly its own text's words;
fallback (proportional) rows carry no walls and persist NULL — nothing
estimates walls into the render.

**D4 — Run-over-run stability.** Same source + same diarization ⇒ the
same (text, span, badge, token-wall) multiset. Consumer enumeration
(explore pass, 2026-11-19): NO harness pins the render column NULL —
the ear-gate snapshot cross-checks `transcript_sources` (untouched),
`full_retranscribe_run` backs both tables up without asserting wall
state, and the persistence oracle uses a sandbox schema; the apply's
stability task RE-VERIFIES this enumeration rather than assuming it.

**D5 — Bright lines inherited.** Token walls are numbers keyed to words
in the LOCAL DB only — nothing meeting-derived enters the repo; records
stay token-only (counts, wall coordinates, sha256); no fixture reads in
production; probes env-gated with loud evidence-home skips; `cargo test
-j 2`; no concurrent cargo during live runs.

**D6 — Handoff contract to the paused change (consumer sketch, no code
here).** With walls persisted, `talkover-word-order-repair`'s attested
route gains a wall-split straddle policy: a straddling render row splits
at the token boundary nearest the attested wall (real words move with
their real audio — the D7 absorb-whole trap dissolves for wall-bearing
rows; no erase, no resurface). Sketch on the S16 geometry: the attested
walls [1024.61,1057.30] split the mega-row [1042.40,1074.42] at a real
token boundary east (occurrence #2's tokens stay in the untouched east
piece) and the row [982.17,1024.75] at a real boundary west; the
separator margins at that window shape are already measured good
(0.578/0.217, occurrence #1 recovered — probe run 1). Two notes carry
over: the pinned change's `render_sha256` content hash SHOULD cover the
wall payload (walls are part of the render's identity; regeneration
re-derives them), and its comparison invariant should reuse the
validator's matching semantics (D3), not a stricter re-definition.

## Risks / Trade-offs

- [Wall smearing at source level] → the source walls are the same ones
  the wall-vote machinery has used since September (validated on the
  reference meeting's real audio); the token-less fallback covers any
  legacy row; integrity invariants catch corruption mechanically.
- [DB growth from per-row wall JSON] → walls are one small JSON blob per
  row in an existing TEXT column; the sources already carry them.
- [Harness/spec drift] → the delta supersedes the "NULL is permanent"
  clause explicitly; harness updates are red-first tasks, not silent
  edits.
- [The paused change's resume depends on this landing] → the handoff is
  an explicit task: re-run the paused change's baseline probe check on
  the new render (the mega-row then carries walls) before its §3
  resumes.
