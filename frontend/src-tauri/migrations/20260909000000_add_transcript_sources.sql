-- transcript_sources: the immutable per-meeting copy of the transcription rows
-- (change `align-from-immutable-source`, design D1/D2).
--
-- The speaker diarization pipeline used to consume its own output: it deleted
-- the meeting's `transcripts` rows and replaced them with aligned rendering
-- rows in the same table, so every Speakers run degraded the text (sentence
-- punctuation, token timestamps, original row boundaries). From now on
-- `transcripts` is the rendering table (what every reader consumes) and
-- `transcript_sources` is the frozen input the pipeline aligns from.
--
-- Column set mirrors the transcription-era columns of `transcripts` exactly
-- (the speaker-lane columns `speaker_label`/`speaker_source`/`previous_label`
-- and the post-insert stamp `continues_previous` are deliberately NOT
-- mirrored — the same columns today's split INSERT omits). `source_origin`
-- keeps the backfill provenance queryable: 'stt' = written by the
-- transcription lane post-split; 'backfilled' = seeded below.
--
-- The FK mirrors `transcripts`; deletion of a meeting goes through the
-- explicit DELETE in `delete_meeting` (house pattern — the cascade is never
-- relied on).
CREATE TABLE IF NOT EXISTS transcript_sources (
    id TEXT PRIMARY KEY,
    meeting_id TEXT NOT NULL,
    transcript TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    summary TEXT,
    action_items TEXT,
    key_points TEXT,
    speaker TEXT,
    audio_start_time REAL,
    audio_end_time REAL,
    duration REAL,
    token_timestamps TEXT,
    source_origin TEXT NOT NULL DEFAULT 'stt',
    FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
);

-- Eager backfill (design D2): every existing meeting's current rows become its
-- frozen source, ids preserved. At migration time provenance is unprovable, so
-- NOTHING is stamped 'stt' — all rows carry 'backfilled'. For never-diarized
-- meetings the copy is exact; for legacy-degraded meetings (already-diarized
-- rows) this freezes the degradation — no further loss — and retranscription
-- is the healing path (it deletes and rewrites both tables).
--
-- The per-meeting count of backfilled rows with NULL `token_timestamps` (the
-- replaced-row signature of prior diarization runs) is logged at upgrade time
-- by the app right after this migration applies (manager.rs) — SQLite cannot
-- emit it from inside the migration.
INSERT INTO transcript_sources
    (id, meeting_id, transcript, timestamp, summary, action_items, key_points,
     speaker, audio_start_time, audio_end_time, duration, token_timestamps,
     source_origin)
SELECT
    id, meeting_id, transcript, timestamp, summary, action_items, key_points,
    speaker, audio_start_time, audio_end_time, duration, token_timestamps,
    'backfilled'
FROM transcripts;
