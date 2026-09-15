# Design: recognized-name revert recovers the cluster label

## D1 — Recovery source: the matched embedding's cluster label

`persist_stamped_centroids` stores one embedding row per cluster per meeting
(`delete_embeddings_by_meeting` first, so the set always mirrors the CURRENT
run) with `cluster_label = "Speaker {index}"` and, on a stamp match,
`speaker_id = <named speaker id>`. The recognized name on transcript rows is
therefore the join key's OTHER side: the original cluster label is recoverable
as

```sql
SELECT DISTINCT cluster_label FROM speaker_embeddings
WHERE source_meeting_id = ? AND speaker_id = (SELECT id FROM speakers WHERE name = ?)
```

No new storage, no schema change.

## D2 — Ambiguity rule: lowest recovered label

If several clusters of one meeting matched the same stamped speaker
(over-split voice), every row carries the SAME name and no per-row cluster
marker exists — a per-cluster restore is not derivable. The rows collapse
onto the lowest recovered cluster label (deterministic, testable) — NUMERIC
order (`CAST(substr(cluster_label, 9) AS INTEGER)`), not lexical, so two-digit
clusters don't beat one-digit ones. This merges one person's over-split rows
under one label — semantically safe (same voice), cosmetically coarser than
the pre-recognition split.

## D3 — Ordering, unlink shape, transactionality

Everything runs in ONE transaction: a partial revert (relabel committed,
unlink lost) could never self-heal — a retry matches 0 rows and skips the
unlink, leaving the recognition live under generic labels. Order within
`revert_speaker_label`:

1. Manual path (unchanged): restore `previous_label`, clear history — captures
   `originals` BEFORE clearing.
2. Recognized path (new): relabel rows still carrying the bare name with
   `previous_label IS NULL` to the recovered label. Manual rows of the same
   name already left in step 1, so no row is touched twice.
3. Symmetric unlink, gated on total rows affected. When step 1 matched
   nothing (`originals` empty) the old `cluster_label IN ()` interpolation
   would emit invalid SQL (`IN ()`), so the empty-originals shape binds only
   `source_meeting_id` and the named-speaker subquery — same unlink
   semantics, reachable today exactly when the recognition is the only thing
   being undone.

## D4 — Scope and non-goals

- No frontend change: the icon heuristic (non-"Speaker "/"Unknown" labels)
  is exactly the set of labels that are now revertible.
- The manual-rename path is unchanged; the never-labeled-row limitation
  (label was NULL before the override AND no matched embedding exists)
  remains and stays documented. Accepted corner (review round 1, minor 5):
  a manual override applied to a NULL-labeled row is swept to the recovered
  cluster label when a matched embedding exists — its prior state was
  unlabeled and `previous_label` stays NULL, so it remains re-labelable.
- Auto-generated labels ("Speaker N") and "Unknown*" remain non-undoable.
