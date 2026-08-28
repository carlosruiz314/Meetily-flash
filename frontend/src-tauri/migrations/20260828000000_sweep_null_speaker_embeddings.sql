-- speaker-identity-embedding-stamping (task 4.2)
-- Legacy diarization runs persisted every centroid with speaker_id = NULL, so
-- the cross-meeting matcher could only pool by cluster label ("Speaker 0" of
-- every meeting colliding). Those rows predate identity stamping and are
-- excluded from the matcher pool regardless; NULL remains a legitimate value
-- only for deliberately unlinked rows (revert, speaker deletion), which the
-- matcher keeps ignoring. Regeneration: re-run Speakers on a meeting.
DELETE FROM speaker_embeddings WHERE speaker_id IS NULL;
