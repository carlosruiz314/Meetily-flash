# Proposal: diarization must align from immutable source rows

## Why

`persist_aligned_groups` DELETES the meeting's transcript rows and replaces
them with the aligned output (same `transcripts` table; no source copy
exists anywhere in the DB). The Speakers pipeline therefore consumes its own
output on every re-run, and each generation irreversibly loses text
structure:

- Measured on cde5c264: the pre-diarization source row "Where is Ricardo?
  I don't know. Let me ping in. I can't." (transcripts.json, [32.51,40.24])
  became, after legacy + driven runs, the fused row "Where is Ricardo
  I don't know." — the "?" after "Ricardo" is gone, so the whole-atom
  aligner can never separate the sentences again; the fused atom goes WHOLE
  to the majority badge (Carlos), misbadging Cynthia's question. Row counts
  drifted across runs (237 source rows → 240 → 188 → 173, later 178 and 182
  under deliberate engine changes); the counts are historical, the mechanism
  is verified in code: each run persists fresh-UUID rows with
  `token_timestamps = NULL` and deletes absorbed rows, so every re-run
  consumes strictly more-degraded input than the last.
- The row-boundary shapes of the "pre-run" backup exactly match the LEGACY
  engine's turn spans ("Where is" / "Ricardo" / "I" / "don't know. ..."),
  proving the destruction started with the app's own earlier runs — the DB
  had not held the true source for weeks.

## What changes

- The transcription source rows become immutable input: a `transcript_sources` table
  (seeded by an eager backfill migration, written by the transcription lane, never
  touched by the speaker lane) holds what the engines produced; diarization reads its
  alignment input AND the engine's transcript-prior timestamps from it on EVERY run.
- The persist step is redesigned as full-meeting regeneration: each run rebuilds the
  meeting's auto rendering in one transaction (manually-corrected rows survive;
  degenerate alignments abort instead of wiping). The current persist cannot survive
  the split — it matches rendering rows by input-row id, so a second run would sweep
  prior output as "absorbed" and re-insert nothing (adversarial-panel finding).
- Sentence punctuation is load-bearing engine input: row merges must never drop it
  (the current `merge_same_label_fragments`/consolidation joins text without
  re-checking sentence integrity).

## Impact

- Without this, every Speakers click silently degrades the transcript text
  and the whole-atom no-split guarantee erodes: a fused atom is assigned
  whole to ONE badge, trading a split-sentence defect for a wrong-badge
  defect. Both bars demand the source survive.
- Scope: speaker pipeline persist path + transcription storage; no engine
  change.
