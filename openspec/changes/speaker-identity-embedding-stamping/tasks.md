## 1. Registry: id-keyed, fresh per run

- [ ] 1.1 RED→GREEN: registry hydration keys vectors by `speaker_id` (not `COALESCE(name, cluster_label)`); setup.rs hydration updated
- [ ] 1.2 RED→GREEN: a Speakers run loads the match pool from the DB at run start — an embedding written after startup (or a rename) is visible to that run
- [ ] 1.3 Add stamped-pool query (`speaker_id IS NOT NULL` join `speakers` for named-only filtering); RED→GREEN test proves auto rows and NULL rows are excluded from the pool
- [ ] 1.4 Replace hard-coded `search(&emb, 0.60)` with the configured match threshold (default 0.40, clamped [0.35, 0.70]); test the clamp

## 2. Live store path: transactional stamped writes

- [ ] 2.1 RED→GREEN: `run_diarization_for_meeting` stores every centroid with non-null `speaker_id` — matched clusters link to the named speaker's id, unmatched link to the meeting-local auto row
- [ ] 2.2 RED→GREEN: delete-stale + insert-stamped runs in one sqlx transaction; an injected insert failure rolls back to the previous stamped set and fails the run (no more `log::warn`-and-continue)
- [ ] 2.3 RED→GREEN: second run on the same meeting replaces the stamped set 1:1 (old ids gone, every new row non-null)
- [ ] 2.4 Confirm zero references to the dead `DiarizationProcessor` remain (deleted by `decommission-queue-diarization-phase`)

## 3. Rename and revert link identity

- [ ] 3.1 RED→GREEN: `label_speaker` links embeddings — candidate set = that meeting's embeddings where `cluster_label` = original label OR `speaker_id` = current speaker id; covers unrenamed, renamed, and auto-matched badges
- [ ] 3.2 RED→GREEN: revert over-unlink case — reverting cluster B does not unlink cluster A's embedding (pins the `cluster_label NOT IN (...)` corruption)
- [ ] 3.3 RED→GREEN: revert under-unlink case — reverting the renamed cluster itself does unlink its embedding
- [ ] 3.4 Rename to a brand-new name creates the speaker row then links; rename to an existing name relinks (matches live "re-label" scenario, this-meeting-only)

## 4. Lifecycle and sweep

- [ ] 4.1 RED→GREEN: deleting a meeting also deletes its `speaker-auto-{meeting_id}-*` speaker rows (named speakers untouched)
- [ ] 4.2 Migration: `DELETE FROM speaker_embeddings WHERE speaker_id IS NULL` (timestamped, transactional)

## 5. Verification

- [ ] 5.1 `cargo test --lib` green (speaker repo + commands suites)
- [ ] 5.2 Live check on cde5c264 via app: after a Speakers run, `speaker_embeddings` rows all non-null; unnamed-meeting auto rows absent from a second meeting's match pool; rename before a run is picked up; threshold change from settings affects auto-labeling
- [ ] 5.3 OpenSpec archive: sync deltas into `openspec/specs/speaker-diarization/spec.md`
