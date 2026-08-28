## 1. Repository layer

- [ ] 1.1 Add `list_stamped_embeddings` (speaker_id IS NOT NULL) to `SpeakerRepository`; RED→GREEN test proves NULL rows are excluded from the pool
- [ ] 1.2 Verify `link_embedding_to_speaker` behavior with a test (links one embedding id to one speaker id)

## 2. Processor: match first, store stamped

- [ ] 2.1 RED→GREEN: processor test asserting every stored centroid row has non-null `speaker_id` after a run with an empty prior pool (new speaker rows created for unmatched clusters)
- [ ] 2.2 Reorder `DiarizationProcessor` to run `match_speakers` on the stamped pool before persistence; matched clusters link to the matched speaker id, unmatched create + link new `speakers` rows (auto-speaker id convention)
- [ ] 2.3 RED→GREEN: second-run test — centroids matching a prior meeting's speaker link to that speaker's id and no duplicate `speakers` row is created
- [ ] 2.4 Point `match_speakers` at `list_stamped_embeddings`; legacy NULL rows contribute nothing (test with NULL rows present)

## 3. Rename links identity

- [ ] 3.1 RED→GREEN: `label_speaker` to an existing speaker's name relinks that meeting's cluster embeddings to that speaker id
- [ ] 3.2 RED→GREEN: `label_speaker` to a new name creates the speaker row, links embeddings, and updates transcript labels as before

## 4. Sweep migration

- [ ] 4.1 Add migration deleting `speaker_embeddings WHERE speaker_id IS NULL` (timestamped, follows existing migration conventions)

## 5. Verification

- [ ] 5.1 `cargo test --lib` green (speaker repository + processor suites)
- [ ] 5.2 Live check: run Speakers on meeting cde5c264 via the app; confirm `speaker_embeddings` rows for that meeting all carry non-null `speaker_id` and the matcher no longer pools by cluster label
- [ ] 5.3 OpenSpec archive: sync spec delta into `openspec/specs/speaker-diarization/spec.md`
