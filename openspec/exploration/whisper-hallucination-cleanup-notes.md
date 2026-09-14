# Exploration: Whisper hallucination cleanup in the transcription lane

Date: 2026-09-09. Evidence meeting: cde5c264 (meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323).

## Q1 — Which code path transcribed this meeting, and with which options

Batch retranscription, not the parallel processor, not the candle `stt.rs` path:

- `use_cases/transcription_queue` → processor closure wired in `lib.rs:683` calls
  `audio::retranscription::start_retranscription(app, meeting_id, folder, None, None, None)`
  — **language = None, model = None, provider = None**. This drops the user's language
  preference entirely: the frontend syncs the pick into `LANGUAGE_PREFERENCE`
  (`ConfigContext.tsx` mount effect; default `auto`), but the queue reads none of it,
  and the engine maps `None` to per-segment detect **without translation**.
- `retranscription.rs::run_retranscription`: decode full `audio.mp4` → 16 kHz mono →
  Silero VAD (`get_speech_chunks_with_progress`, redemption 2000 ms) → split >25 s
  segments at silence → per segment
  `WhisperEngine::transcribe_audio_with_confidence(samples, language, offset_ms)`.
- Model: `large-v3`, read from `transcript_settings` (`provider=localWhisper, model=large-v3`,
  verified read-only against a copy of the live DB).
- Engine options actually set (`whisper_engine.rs::transcribe_audio_with_confidence`):
  BeamSearch{beam_size from hw profile, patience 1.0}, `set_language(None)` (auto-detect),
  `suppress_blank=true`, `suppress_nst=true`, `set_temperature(0.1..0.4 per hw tier)`,
  `max_initial_ts=1.0`, `entropy_thold=2.4`, `logprob_thold=-1.0`, `no_speech_thold=0.55`,
  `max_len=200`, `audio_ctx` clamped to segment length.

## Q2 — Are the anti-hallucination gates wired?

Verified against the vendored whisper.cpp in `whisper-rs-sys-0.15.0` (whisper-rs 0.16.0):

- `no_speech_threshold`: exposed (`set_no_speech_thold`), set to 0.55 (default 0.6).
- `compression_ratio_threshold`: **does not exist in this whisper.cpp fork** — replaced by
  `entropy_thold` ("similar to OpenAI's compression_ratio_threshold", whisper.h:547).
  It IS set (2.4 = the default). The comment in `whisper_engine.rs:722` is right about
  API availability (no `set_compression_ratio_threshold` in whisper-rs 0.16) but stale
  in its implication that no repetition gate is engaged — `entropy_thold` is that gate
  and it is set. Task 2.3 appends the clarifying note; nothing is removed.
- `logprob_threshold`: exposed, set to −1.0 (default −1.0).
- Temperature fallback: active. `temperature_inc` is never set by the app, so the default
  0.2 builds a ladder `[t_base, t_base+0.2, … < 1.0]` (whisper.cpp src/whisper.cpp:6854).
  The ladder applies to beam search too, and the fallback decision is strategy-independent.
- Gate semantics (whisper.cpp src/whisper.cpp:7527, 7555): a decode fails if
  `result_len > 32 && entropy < 2.4`, or if `avg_logprobs < −1.0 AND no_speech_prob < 0.55`.
  **Critical hole: when every ladder temperature fails, whisper.cpp still emits the last
  decode's text** (output block after the loop). Failed gates never produce an empty result.

### Why garbage survives all of that

1. **Per-segment language auto-detect, no translation.** `language = None` from the
   queue → each VAD segment independently detects its language and decodes with no
   translate. Short crosstalk/laughter windows detect CJK/Korean/Cyrillic and the
   foreign-script text passes through untranslated. (The `LANGUAGE_PREFERENCE` static
   in `lib.rs:73` holds the UI's pick — default `auto` after the frontend startup sync,
   Rust default `auto-translate` before it — but the batch queue reads neither; the
   static's other readers are the `whisper_transcribe_audio` command and the retired
   parallel path.)
2. **VAD only removes silence** (0.50/0.35 thresholds, 250 ms min speech, 2000 ms
   redemption). Laughter/crosstalk/noise passes as speech; on that input Whisper
   hallucinates with *good* logprobs and low `no_speech_prob`, so the logprob gate
   (fires only when `avg_logprobs < −1.0` AND `no_speech_prob < 0.55`) does not fire,
   and when every ladder temperature fails, the text is emitted anyway (hole above —
   in this low-`no_speech_prob` regime gate failure never yields an empty result; the
   high-`no_speech_prob` regime does suppress the window).
3. **`clean_repetitive_text` collapses pure loops to harmless remnants** (a 39-token
   "Okay." row collapses under today's filter — the loop rows in the fixtures predate
   or bypassed it) — but it never *flags* partial loops inside mixed rows and has
   English-only meaningless patterns, so multilingual salad passes untouched.

## Q3 — Detector prototype and validation

`hallucination-detector-probe.py` (this directory). Text-only, per row:

- `fffd ≥ 1` — U+FFFD replacement chars (from lossy token decode) — never occurs in
  legitimate rows.
- `non_latin ≥ 3` — chars whose Unicode script class is CJK/Hangul/Cyrillic/Greek/Thai/
  Arabic/… (Latin-accented chars deliberately not counted; an English meeting may carry
  é/ñ legitimately in names).
- `loop_ratio ≥ 0.5` **and** top-token count ≥ 10 — most frequent whitespace token /
  total tokens, rows with ≥6 tokens (catches "Okay. ×37", "Yes. ×50"; a 6× unpunctuated
  "yeah yeah yeah yeah yeah yeah" backchannel stays under the count gate and does not
  flag).
- `wps ≥ 25` — words/sec on rows ≥0.8 s (absurd decode rate; never fired alone — insurance).

Validation (both pinned fixtures; rows carry `id,text,start_ms,end_ms`):

| fixture | rows | flagged | flagged audio | unflagged rows w/ any salad signal |
|---|---|---|---|---|
| cde5c264_transcripts.pre-live.json | 240 | 22 (9 %) | 368 s | **0** |
| cde5c264_transcripts.json | 173 | 15 (9 %) | 671 s | **0** |

Notes on the counts: the task brief said "20 of 240 / 13 of 13-173". The detector finds
two additional rows per fixture — the "Yes. Yes. ×50" and "Okay. ×37" loop rows, which are
repetition degeneration by the same evidence standard (50 identical tokens in 22 s),
plus rows where salad is embedded mid-row inside mostly-legit English text (e.g. current
row [77]: 86 s, 190 non-Latin chars interleaved with real sentences). Separation is clean:
**no unflagged row contains a single non-Latin char, U+FFFD, or dominant loop.**

Implementation home: new `src/audio/hallucination.rs`, called from the retranscription
loop after decode (`transcribe_segments_checkpointed` transcribe closure) — app layer,
never the speaker lane (alignment only reads rows). Unit tests pin both fixture paths
and skip gracefully when the untracked fixture files are absent.

## Q4 — Per-row policy

Recommendation: **flag → re-transcribe the window once with pinned language → drop only
on second failure.**

- Dropping flagged rows outright loses real content: many flagged segments are mostly
  legitimate English with embedded salad (e.g. current [110] "You know, our boss, my boss
  is often right…" + Cyrillic chunk). The LLM summary consumes these rows, so garbage
  poisons downstream output; but wholesale drops of mixed rows punch holes in the record.
- Re-transcription with `language=Some("en")` (and the same model) kills the per-segment
  auto-detect that caused the salad in the first place; windows total ~11 min for
  cde5c264 — sample-sliced from a single full decode of `audio.mp4`, no full re-run.
- A row that still fails the audit after the pinned-language retry is unrecoverable
  noise → drop + log (cleanliness beats completeness for an unrecoverable span; the
  alternative — emitting a `[unintelligible]` marker row — is available if completeness
  matters more later).
- U+FFFD rows are decode corruption; the retry resolves them the same way.

For pre-existing rows (cde5c264 today): a one-off offline repair harness (test-style, like
`tests/live_speakers_run.rs`) that audits the meeting's rows, re-transcribes flagged
windows, and rewrites those DB rows in one transaction. Diarization re-runs afterwards via
the existing `run_diarization_for_meeting` invocation — no speaker-lane code changes.

## Root-cause fix ordering (for the change)

1. Resolve batch language once per run honoring the UI contract (`auto` /
   `auto-translate` / unset → `auto-translate` passed through; explicit code → pinned),
   pass it into the queue's `start_retranscription` call — removes the no-translate,
   no-pin path that produced the foreign-script rows.
2. Post-decode audit + one DIFFERENT-decode retry (greedy temperature-0, language
   pinned — a same-params retry reproduces flagged text verbatim) + drop as the safety
   net for degeneration that survives translation (loops like "Okay. ×37").
3. Repair harness for already-stored meetings.

## Adversarial panel round 1 — corrections applied

Three-adversary panel (facts / robustness / OpenSpec process), round 1 findings folded
into the change docs:

- **False premise removed**: the original draft claimed "the frontend never calls
  `set_language_preference`". It does — `ConfigContext.tsx` syncs on mount with default
  `auto`. D1 was reworked from "map the `auto-translate` default → `en`" (a no-op for
  default users, and a silent redefinition of the translate contract) to passing
  `auto-translate` through, which honors the picker's promise and neutralizes the
  startup-sync race.
- **Retry determinism**: a same-params retry is a byte-identical repeat under beam
  search; the quarantine retry is now specified as greedy/temperature-0 with the
  language pinned — a genuinely different decode.
- **Acceptance 5.2 reworded**: "zero flags after repair" was only satisfiable vacuously
  (loop rows may still loop under pinned-en). Now: each former garbage row is
  `repaired-clean` or `dropped`, counts and dropped text reported, zero flagged rows
  remain.
- **Repair write safety**: backup dump of original rows before the transaction, app
  closed, `token_timestamps`/speaker label reset on replaced rows.
- **Circuit breaker** (drops > 30 % ⇒ job Failed) added against gutted-transcript
  publication; `SHOULD_YIELD` re-checked before retries; resume-path audit specified
  (flagged checkpoint → retry → replace or delete the checkpoint row).
- **Loop trigger tightened** (ratio ≥ 0.5 AND top-token count ≥ 10) so 6× unpunctuated
  "yeah" backchannel can't flag; fixture counts unchanged (22/15), expectations now
  tracked in `tools/expected-flags.json` keyed by `row_sha256`.
- **Sequencing**: the cde5c264 repair run waits for `no-split-sentences` to archive
  (it freezes cde5c264 behind a SHA-pinned snapshot), then re-pins the snapshot.
- **Facts tightened**: gate-failure-never-empty qualified to the low-`no_speech_prob`
  regime; `clean_repetitive_text` described as collapsing pure loops (fixture loop rows
  predate/bypass it) rather than "loops survive"; the initial_prompt clause dropped
  (nothing sets one); `LANGUAGE_PREFERENCE` reader list corrected (also
  `whisper_transcribe_audio`); decommission change given a validating delta, plain
  deletion phrasing, repo-relative paths, and a stale-doc task (`CLEANUP_PLAN.md`).
