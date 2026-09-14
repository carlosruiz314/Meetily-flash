# Handoff: speaker-attribution quality on cde5c264 (post-heal substrate)

For the speaker-lane session (engine-boundary-and-identity-accuracy /
speaker-identity-embedding-stamping / tune-vad-rnnoise). The transcription
lane's heal (whisper-hallucination-cleanup) is done and validated; the
principal reports label MISATTRIBUTIONS (not choppiness, not sentence
splits) on the healed meeting. Evidence packet:

## What changed under you

The full re-transcription (2026-09-14) replaced the alignment substrate:
- `transcript_sources`: 182 degraded rows → 229 fresh rows (new VAD spans,
  fresh token-timestamp JSON, `source_origin='stt'`), sha `b0860e2a…`.
- One Speakers re-derive (`run_speakers_reset_standalone`, threshold 0.65
  from settings) re-labeled from scratch: 2 voices, 374 segments labeled,
  209 consolidated rendering rows, badges Cynthia Wu 108 / Speaker 0 101.
- All your landed invariants held: source byte-identical across the run;
  no rendering gaps/overlaps; zero hallucination-flagged rows.

## Why re-tuning is expected

Your earlier tuning (gap-rescue v3 sub-window scans, sentence-aware turns,
thresholds) was calibrated against the OLD 182-row geometry. The new rows
have different spans and boundaries, so span-specific calibrations no longer
land where they did. The label BEHAVIOR, however, is unchanged pre/post heal
(flip rate 98% → 93%; same text attributed the same way at the meeting
close), so nothing regressed — this is a re-calibration pass, not a
regression hunt.

## Concrete leads

1. "Speaker 0" (101 rows) has no registry entry — only Cynthia's voice is
   enrolled. Misattribution complaints may partly be naming, not clustering.
2. `speakerMergeThreshold` = 0.65 (settings) was applied. If two real voices
   are being merged or one voice is splitting, this is the first dial.
3. Sample stretches the principal read (rendering spans, healed text):
   - 4586–4613 s: "Oh, oh, I understand. Right." + "So we need to find a
     sweet spot…" — both under Speaker 0; the "I understand / Right."
     exchange pattern suggests two voices in close alternation.
   - 4612.8–4683.2 s: a single 70.4 s Cynthia row (run-assembly
     consolidation of a long same-speaker run) — verify the consolidation
     didn't absorb the other voice's interjections.
   - 4879.8–4934.9 s "And they don't know your larger strategy…" → Cynthia;
     4934.9–4946.6 "…I need to drop…" → Speaker 0. Principal knows the truth.
4. Ground truth is available from the principal: he can name the true
   speaker for specific spans on request.

## Restore points (if you need the pre-heal world)

- DB backup: `%APPDATA%/com.meetily.ai/meeting_minutes.retranscribe-backup-20260914-170533.json`
  (both tables; texts + labels intact, time columns null — CAST aliasing bug
  in the backup dumper).
- Folder backup: `transcripts.pre-retranscribe-20260914-170533.json`
  (older 237-row segmentation era with times, different row ids).
- Fixture era pins: `ad1ebbb7` (pre-live, 240 rows, 22 flagged) and
  `b0860e2a` (current, 229 rows, 0 flagged).
