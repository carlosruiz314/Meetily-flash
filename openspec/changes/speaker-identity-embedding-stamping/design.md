## Context

`DiarizationProcessor` computes one centroid per cluster and stores it via `SpeakerRepository::store_embedding(pool, emb_id, None, centroid, meeting_id, cluster_label)` — the speaker id is hard-coded `None` (`use_cases/diarization_processor.rs`). `match_speakers` runs afterwards and returns a `label_map`, but nothing feeds its result back into the stored rows. The repository already ships the intended linking step (`link_embedding_to_speaker`) with zero callers, and `run_diarization_for_meeting` already deletes a meeting's stale embeddings before processing. Rename (`labelSpeaker`) updates transcript labels and the `speakers` table but never touches embeddings. Net effect: the matcher's only usable pool key is the per-meeting cluster label, which is why "Speaker 0" collides across meetings.

## Goals / Non-Goals

**Goals:**

- Every new embedding row carries a real `speakers.id`.
- Cross-meeting matching pools only stamped rows, keyed by speaker id.
- Renaming a cluster links that meeting's embeddings to the named speaker, so user corrections become identity, not just labels.
- Legacy NULL rows become inert (ignored) and are swept in one migration.

**Non-Goals:**

- No change to clustering, thresholds, transcript labels, or the Speakers-button flow.
- No voice enrollment / name-from-clip feature (that remains a UI rename action).
- No backfill of the NULL rows — they are derived data, regenerable by re-running Speakers.

## Decisions

- **Match first, then store stamped.** Reorder the processor: load the stamped pool, run `match_speakers`, then persist centroids with the resolved speaker id (matched → existing id; unmatched → `create_speaker` with the existing auto-speaker id convention, then link). Alternative rejected: storing NULL and linking afterwards — keeps a window where NULL rows exist and re-introduces the sweep problem on every run.
- **Matcher pool = stamped rows only.** Add `list_stamped_embeddings` (or a `speaker_id IS NOT NULL` variant of `list_all_embeddings`) and point `match_speakers` at it. Alternative rejected: deleting NULL rows alone — fixes the pool today but the next run recreates NULLs.
- **Rename links embeddings.** When `label_speaker` renames a cluster to an existing speaker's name, that meeting's embeddings for the cluster are relinked to that speaker (`link_embedding_to_speaker` finally gets its caller). Renaming to a brand-new name creates the speaker row first. This is what makes "host is Cynthia" stick across meetings; without it, identity is only whatever the auto-matcher guessed.
- **Legacy sweep via one migration.** `DELETE FROM speaker_embeddings WHERE speaker_id IS NULL` in a timestamped migration. No backfill: centroids are cheap derivatives of audio the user can re-run.
- **Naming of auto-created speakers** keeps the current "Speaker N" placeholder so the rename UI's known-speaker autocomplete (which filters out `Speaker *` names) does not offer unresolved identities.

## Risks / Trade-offs

- [Processor reorder touches the diarization hot path] → unit tests around `match_speakers`/persistence exist; the reorder is covered by a RED→GREEN test that stored rows must carry non-null ids.
- [Strict matching threshold fragments one person into several speaker rows across meetings] → user rename is the correction path and now relinks embeddings; threshold tuning stays out of scope.
- [Rename-to-existing-name now has side effects beyond labels] → scoped to the renamed meeting's cluster rows only; revert is a re-run of Speakers.
- [Migration deletes data] → rows are regenerable per meeting via Speakers; no transcript impact.

## Migration Plan

1. Ship code (stamping + stamped-only pool + rename linking) with tests.
2. Migration sweeps `speaker_id IS NULL` rows.
3. User regenerates identity per meeting by clicking Speakers (per-meeting delete makes this idempotent).
4. Rollback: revert code; stamped rows remain valid data; a run re-creates previous behavior only if reverted.

## Open Questions

- Should auto-created `speakers` rows be pruned if a meeting is deleted? (`remove_auto_speakers_for_meeting` suggests the convention exists — reuse it rather than inventing lifecycle.)
- Identity-match similarity threshold: reuse the existing `match_speakers` threshold unchanged unless live behavior shows fragmentation.
