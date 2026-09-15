# Proposal: recognized speaker names must be revertible to their cluster label

## Why

The badge undo icon shows on every non-auto-generated speaker label, but for a
RECOGNIZED name — diarization matched a stamped speaker and wrote the person's
name directly — clicking it was a silent no-op:

- `revert_speaker_label` only restored rows carrying a `previous_label`
  (the manual-rename path). Recognized-name rows are written by
  `persist_regenerated_rendering` with `previous_label = NULL`, so the UPDATE
  matched 0 rows, returned `Ok(0)` (success), and the refetch rendered
  identical data — nothing visible happened.
- Measured on cde5c264: 108 rows labeled "Cynthia Wu", all `previous_label
  IS NULL` — the undo affordance was offered and did nothing.

## What changes

- `revert_speaker_label` gains a recognized-name recovery path: the cluster
  label a run stored on the matched embedding
  (`speaker_embeddings.cluster_label`, joined via `speakers.name`) is
  recovered and every row carrying the bare name in the meeting
  (`previous_label IS NULL`) is relabeled to it (`speaker_source = NULL`).
  If several clusters of the meeting matched the same name (over-split
  voice), the rows are indistinguishable per-row and collapse onto the
  lowest recovered cluster label.
- The symmetric unlink extends to the recovery-only case (an empty
  `cluster_label IN ()` list would be a SQLite syntax error, so the
  no-originals shape omits the IN clause).
- NO frontend change: the badge heuristic (hide undo on "Speaker "/"Unknown"
  prefixes) already offers undo on every non-default name; the icon was
  dead for recognized names only because the backend had no recovery path.
  An interim attempt to gate the icon on label history was fully reverted.

## Impact

- Undo becomes truthful for recognized names: one click always lands on a
  "Speaker X" label and unlinks the recognition, so future meetings stop
  auto-applying that name. Scope: the revert path in the speaker repository
  only — no engine, persist, or labeling-path changes; the manual-rename
  revert path is byte-identical to before.
