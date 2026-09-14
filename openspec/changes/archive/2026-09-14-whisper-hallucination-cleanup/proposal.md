## Why

Meeting cde5c264's transcripts contain Whisper hallucination rows: multilingual word
salad (CJK/Korean/Cyrillic in an English meeting), U+FFFD replacement chars, and
repetition loops ("Okay. ×37", "Yes. ×50"). The pinned fixtures capture 22 of 240 rows
(pre-live) and 15 of 173 rows (current) as garbage — ~9 % of rows, clustered in dense
banter/noise windows (~866 s, ~1842–1943 s, ~2153–2207 s, ~2821 s, ~3370 s, ~4558–4595 s,
~4880 s). These rows come out of the transcription itself; the speaker/alignment lane
never edits words, so the fix lives in the transcription lane.

Root cause chain (verified, see `openspec/exploration/whisper-hallucination-cleanup-notes.md`):

1. The transcription queue calls `start_retranscription(..., language=None, ...)`,
   dropping the user's language preference entirely. The engine maps `None` to
   per-segment language auto-detection **without translation** — losing both the UI's
   "Auto Detect (Translate to English)" contract and any explicit language pin. Short
   crosstalk/laughter windows detect the wrong language and decode into
   CJK/Korean/Cyrillic.
2. whisper.cpp's built-in gates (entropy 2.4, logprob −1.0, no_speech 0.55, temperature
   ladder) are all wired and active — but they are per-30 s-window gates, and in the
   hallucination regime (noise ⇒ low `no_speech_prob`, plausible logprobs) they pass
   the degenerate decode; when every ladder temperature fails, whisper.cpp still emits
   the final decode. Gate failure in this regime never yields an empty result, so
   degenerate text reaches the DB.
3. The existing `clean_repetitive_text` post-filter collapses pure loops to harmless
   remnants — the loop rows in the fixtures predate or bypassed it — but it neither
   flags partial loops inside mixed rows nor any multilingual salad. Nothing audits
   text before persistence.

## What Changes

- **Batch runs honor the language preference**: the queue resolves the preference
  (`auto`/`auto-translate`/unset → `auto-translate` passed through; explicit code →
  pinned) and passes it to `start_retranscription` instead of `None`.
- **New `src/audio/hallucination.rs` audit** (text-only, no audio): flags U+FFFD,
  non-Latin-script chars (≥3), dominant token loops (ratio ≥0.5 with ≥10 occurrences of
  the top token), absurd rate (≥25 words/s). Tracked expectations file
  (`tools/expected-flags.json`, keyed by the fixtures' `row_sha256`) pins the flag sets.
- **Post-decode quarantine in the batch loop**: a flagged segment is retried ONCE with
  a deliberately different decode (greedy temperature-0, language pinned — a same-params
  retry would reproduce the flagged text verbatim); still-flagged segments are dropped
  from the persisted set, counted, and logged. A per-run circuit breaker (drops > 30 %
  of segments) fails the job instead of persisting a gutted transcript. Resume audits
  checkpointed text and deletes still-flagged checkpoint rows.
- **Offline repair harness** (terminal-only): audits a stored meeting's rows,
  re-transcribes flagged windows from the meeting audio with the different-decode retry,
  report-only by default; `REPAIR_WRITE=1` backs up the original rows to JSON, then
  rewrites only the replaced rows (text; `token_timestamps` and speaker label reset) in
  one transaction with the app closed — writing both the immutable source rows and the
  rendering rows (depends on `align-from-immutable-source`, see Sequencing).
- **Comment-only correction** in `whisper_engine.rs`: the `compression_ratio_threshold`
  note stays (correct for this binding) and gains the missing context — the repetition
  gate here is `entropy_thold`, and the temperature-fallback ladder is active via the
  default `temperature_inc`.

## Capabilities

### New Capabilities

### Modified Capabilities

- `post-meeting-pipeline`: ADDED requirement — batch transcription resolves the
  language preference once per run (never per-segment `None`), audits every segment for
  hallucination, and quarantines flagged segments (differently-decoded retry, then
  drop, with a circuit breaker).

## Impact

- Code: `src/audio/hallucination.rs` (new), `src/audio/retranscription.rs` (audit +
  retry in the checkpointed loop), `src/lib.rs` (queue wiring passes resolved language),
  `src/whisper_engine/whisper_engine.rs` (comment-only), `tests/` (fixture-pinned audit
  tests + repair harness), `.gitignore` (fixture pattern), `tools/expected-flags.json`
  (tracked).
- Acceptance: the audit flags exactly the tracked expected sets on both pinned fixtures
  (zero clean-row flags); the offline repair run on cde5c264 ends with zero flagged
  rows — each former garbage row repaired-clean or dropped, with counts and dropped
  text reported.
- Sequencing: the cde5c264 repair run (task 5.2) waits until `no-split-sentences`
  archives (it freezes cde5c264 behind a SHA-pinned snapshot), then re-pins that
  snapshot. It ALSO waits on `align-from-immutable-source` landing: the repair-write
  path must target the immutable source rows (`transcript_sources`), or the next
  Speakers run re-aligns from the still-garbage source and resurrects the flagged rows.
  Tasks 1–3 are independent of that change. No speaker/diarization-lane changes.
  Fixture paths stay untracked; a `.gitignore` entry enforces it.
