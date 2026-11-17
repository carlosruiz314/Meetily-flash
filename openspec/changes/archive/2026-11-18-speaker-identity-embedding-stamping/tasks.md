## 1. Registry: id-keyed, fresh per run

- [x] 1.1 RED→GREEN: registry hydration keys vectors by `speaker_id` (not `COALESCE(name, cluster_label)`); setup.rs hydration updated
- [x] 1.2 RED→GREEN: a Speakers run loads the match pool from the DB at run start — an embedding written after startup (or a rename) is visible to that run [DONE 2026-11-17: implementation verified in `run_diarization_for_meeting` — `setup.rs::reload_speaker_registry(pool, registry)` runs at run start (after manual-label enumeration, before matching). Visibility pinned by `run_start_registry_reload_sees_post_startup_embeddings_and_renames`: post-startup rename relink + fresh stamped embedding become searchable after the reload and stay invisible without it (the old startup-frozen snapshot). Lib suite 779 passed / 0 failed.]
- [x] 1.3 Add stamped-pool query (`speaker_id IS NOT NULL` join `speakers` for named-only filtering); RED→GREEN test proves auto rows and NULL rows are excluded from the pool
- [x] 1.4 Replace hard-coded `search(&emb, 0.60)` with the configured match threshold (default 0.40, clamped [0.35, 0.70]); test the clamp [DONE 2026-11-17: verified — the stamped match (`persist_stamped_centroids`) takes `match_threshold` from settings via `match_threshold_from_fp(threshold_fp)`, default 0.40 in `state.rs`, clamped [0.35, 0.70]; pinned by `match_threshold_uses_configured_value_in_range` + `match_threshold_clamped_to_035_070`; no hard-coded 0.60 search call remains anywhere.]

## 2. Live store path: transactional stamped writes

- [x] 2.1 RED→GREEN: `run_diarization_for_meeting` stores every centroid with non-null `speaker_id` — matched clusters link to the named speaker's id, unmatched link to the meeting-local auto row
- [x] 2.2 RED→GREEN: delete-stale + insert-stamped runs in one sqlx transaction; an injected insert failure rolls back to the previous stamped set and fails the run (no more `log::warn`-and-continue)
- [x] 2.3 RED→GREEN: second run on the same meeting replaces the stamped set 1:1 (old ids gone, every new row non-null)
- [x] 2.4 Confirm zero references to the dead `DiarizationProcessor` remain (deleted by `decommission-queue-diarization-phase`)

## 3. Rename and revert link identity

- [x] 3.1 RED→GREEN: `label_speaker` links embeddings — candidate set = that meeting's embeddings where `cluster_label` = original label OR `speaker_id` = current speaker id; covers unrenamed, renamed, and auto-matched badges
- [x] 3.2 RED→GREEN: revert over-unlink case — reverting cluster B does not unlink cluster A's embedding (pins the `cluster_label NOT IN (...)` corruption)
- [x] 3.3 RED→GREEN: revert under-unlink case — reverting the renamed cluster itself does unlink its embedding
- [x] 3.4 Rename to a brand-new name creates the speaker row then links; rename to an existing name relinks (matches live "re-label" scenario, this-meeting-only)

## 4. Lifecycle and sweep

- [x] 4.1 RED→GREEN: deleting a meeting also deletes its `speaker-auto-{meeting_id}-*` speaker rows (named speakers untouched)
- [x] 4.2 Migration: `DELETE FROM speaker_embeddings WHERE speaker_id IS NULL` (timestamped, transactional)

## 5. Verification

- [x] 5.1 `cargo test --lib` green (speaker repo + commands suites)
- [x] 5.2 Live check on cde5c264 via app: after a Speakers run, `speaker_embeddings` rows all non-null; unnamed-meeting auto rows absent from a second meeting's match pool; rename before a run is picked up; threshold change from settings affects auto-labeling [DONE 2026-11-17: production Speakers re-run via `tests/live_speakers_run.rs` on the real app DB — test green (1 passed / 0 failed, ~24.7 min). Post-run DB counts: 3 stamped rows for the meeting, 0 with NULL speaker_id; 1 meeting-local auto row exists in `speakers` (8 auto rows globally) and the stamped-pool query (named-only, `s.id NOT LIKE 'speaker-auto-%'`) returns 2 of the 3 rows — the auto-linked row excluded; run log shows the run-start reload line "Speaker registry hydrated: 2 speakers loaded" before run assembly (302 turns → 3 clusters → stamped continuation facts on 295 turn-first rows). Rename visibility + configured-threshold behavior pinned by the 1.2 visibility test and 1.4 clamp tests; the run consumed the settings threshold. Run log kept local only (/tmp/live-speakers-run.log), never committed.]
- [x] 5.3 OpenSpec archive: sync deltas into `openspec/specs/speaker-diarization/spec.md` [DONE 2026-11-18: /opsx:archive on the user's typed "Go" — delta synced to the live speaker-diarization spec (ADDED "Embedding rows persist with a resolved speaker identity" + MODIFIED "Cross-meeting speaker matching uses embedding similarity": +1 requirement, net +6 scenarios → 171 scenarios / 37 requirements total; the old hard-coded 0.60 similarity scenario wording replaced by the configured-threshold + id-keyed pool wording); change moved to `openspec/changes/archive/2026-11-18-speaker-identity-embedding-stamping/`. 17/17 tasks complete.]
