# Design — whisper-hallucination-cleanup

Exploration evidence: `openspec/exploration/whisper-hallucination-cleanup-notes.md`.
Round 1 adversarial panel corrections are folded in (see the corrections section of the
notes).

## Context

Batch retranscription (`transcription_queue` → `retranscription.rs` →
`WhisperEngine::transcribe_audio_with_confidence`) is the only producer of transcript
rows (batch-only architecture). It runs with `language=None` → per-segment
auto-detection **with no translation**; the UI's language contract ("Auto Detect
(Translate to English)" default, or an explicit code) never reaches the batch queue.
whisper.cpp's decode gates emit text even when every temperature-fallback attempt fails
them (in the low-`no_speech_prob` hallucination regime — the high-`no_speech_prob`
regime does suppress the window). The detector prototype is validated: 22/240 and
15/173 rows flagged on the pinned fixtures, zero unflagged rows carry any salad signal.

## Decisions

### D1 — Language: honor the UI's language preference in batch runs

The frontend syncs the user's pick into the Rust static `LANGUAGE_PREFERENCE`
(`lib.rs:73`) on startup (`ConfigContext.tsx` mount effect) and on every change; the
pick is one of `auto` ("Auto Detect (Original Language)"), `auto-translate` ("Auto
Detect (Translate to English)"), or an explicit ISO code. The batch queue reads none of
it — it passes `None`, which the engine maps to per-segment detect **without
translate**. That drops both halves of the user's contract: the translate-to-English
posture and any explicit language pin.

`resolve_batch_language(pref: Option<&str>) -> Option<String>`:

- `None`, `Some("auto")`, `Some("auto-translate")` → `Some("auto-translate")` — passed
  through to the engine unchanged (detect + translate to English; near-identity for an
  English meeting, English minutes for a non-English one, exactly what the picker
  promises).
- `Some(code)` → `Some(code)` — the user asserted the language; every segment decodes
  with it, no per-segment detect.

Rationale for passing `auto-translate` through instead of pinning `en`: pinning `en`
would re-decode genuinely non-English meetings as mangled English and silently redefine
what the picker option means. Translate keeps the contract; the quarantine (D3) handles
the degeneration that survives translation. Both automatic states mapping to the same
engine input also makes the startup-sync race immaterial: a job that runs before the
webview mounts (static still at its Rust default `auto-translate`) and one that runs
after (static synced to `auto`) decode identically. A job paused across an app restart
can still straddle a preference *change*; that yields per-segment text differences on
resume, which checkpoints already tolerate (they match on timestamps). Accepted and
noted.

Unit test pins the mapping; no new settings column (if a persisted Rust-side
transcription-language setting lands later, it replaces the static read).

### D2 — Detector: app-layer, text-only, threshold table

New `src/audio/hallucination.rs`:

```
pub struct HallucinationReport { pub is_garbage: bool, pub non_latin: usize, pub fffd: usize,
                                 pub loop_ratio: f32, pub loop_count: usize, pub wps: f32 }
pub fn audit(text: &str, start_ms: f64, end_ms: f64) -> HallucinationReport
```

Triggers (calibrated on the fixtures; expectations tracked in
`tools/expected-flags.json` keyed by the fixtures' `row_sha256`):

- `fffd ≥ 1` — U+FFFD never occurs in legitimate rows on the fixtures.
- `non_latin ≥ 3` — chars whose Unicode script is CJK/Hiragana/Katakana/Hangul/Cyrillic/
  Greek/Thai/Arabic/… via the `unicode-script` crate (Rust std has no script data; a
  hand-rolled range table would drift from the validated Python prototype, which uses
  `unicodedata.name`). Latin-accented letters excluded, so "é/ñ" in names never trip it.
- token loop: `loop_ratio ≥ 0.5` **and** the top token occurs **≥ 10 times** — "Okay."
  ×37 and "Yes." ×50 flag; a 6× "yeah yeah yeah yeah yeah yeah" backchannel (real
  speech, punctuated variants split tokens and never reach the ratio) does not.
- `wps ≥ 25` on rows ≥ 0.8 s — insurance; never fired alone on the fixtures.

Why app-layer: the engine is shared with other callers and the speaker lane reads rows
from the DB; the audit must gate persistence, not transcription itself. Script-class
detection beats a raw non-ASCII ratio because em-dashes/quotes in legitimate rows
poison byte-ratio heuristics; on the fixtures the separation is total (no unflagged row
contains even one non-Latin char).

Known residual risk, documented rather than tuned away: heavy CJK *quoting* in an
English meeting ("we ship 抖音有道 support") can flag. Mitigation is D3's retry — a
genuinely different decode, not a repeat — plus the drop counter and circuit breaker
below; mass false flags abort the run instead of emptying the transcript.

Test pinning: integration tests read
`tests/fixtures/cde5c264_transcripts{,.pre-live}.json` and compare flagged row ids
against `tools/expected-flags.json` (keyed by `row_sha256`, so a re-pinned snapshot
fails loudly instead of silently passing). If the untracked fixture files are absent,
the test skips with a printed notice.

### D3 — Quarantine in the batch loop: retry must differ, then drop

Inside `transcribe_segments_checkpointed`'s transcribe step (post-decode,
pre-accumulate):

1. `audit(text, start_ms, end_ms)` — clean ⇒ proceed as today.
2. Flagged ⇒ **one retry with a deliberately different decode**: greedy sampling
   (`Greedy { best_of: 5 }`, temperature 0.0, the default `temperature_inc` ladder),
   language pinned to a concrete code (`en` when the run resolved `auto-translate`,
   else the run's code). The first attempt was beam search with auto-detect; a same-
   params retry would reproduce the flagged text byte-for-byte (beam search is
   deterministic), so the retry must vary the search. Samples are already in memory —
   this is a second decode of one ≤ 25 s segment, never a re-decode of the file.
   `SHOULD_YIELD` is re-checked before the retry so a pause request isn't held off by
   an extra decode.
3. Retry clean ⇒ use the retry text (and store it in the checkpoint).
4. Retry still flagged ⇒ drop: not accumulated, no checkpoint written, counted, and
   logged with timestamps + trigger stats.

**Resume path**: the checkpoint-skip branch audits `cp.text` too. Flagged checkpoint ⇒
retry from samples (same rule); clean ⇒ replace the checkpoint row's text; still
flagged ⇒ delete the checkpoint row and drop. A fresh-run drop never creates a
checkpoint row; a resume-drop deletes the stale one.

**Circuit breaker**: if drops exceed 30 % of the run's segments, the job fails instead
of persisting a gutted transcript (the save path `DELETE`s all rows before inserting —
a pathological meeting would otherwise publish an empty transcript and chain to
summary). Threshold is a `const`, logged.

Completeness vs cleanliness: outright dropping wastes the legitimate English embedded
in mixed rows; keeping garbage poisons the summary LLM. Retry-first preserves content;
drop-on-second-failure bounds the cost at ≤ 2 decodes per bad segment (~11 min of
flagged windows for cde5c264, worst case).

### D4 — Repair harness for already-stored meetings

`tests/hallucination_repair_run.rs` (same pattern as `tests/live_speakers_run.rs`),
env-driven, terminal-only:

- Inputs: meeting DB path, meeting folder (audio + rows), language override. Default
  mode is **report-only**: audit rows, sample-slice flagged windows from a single
  decode, retry each with D3's different-decode rule, print before/after. The audit
  reads `transcript_sources` (the durable truth) once that table exists — a
  rendering-only audit is blind to source-only garbage (absorbed/duplicate-resolved
  rows exist only in source), and post-split the next Speakers run re-derives the
  rendering from exactly that table; pre-split (single table) it audits `transcripts`
  as today. Flagged source rows are reported against the rendering rows they map to by
  span overlap (for human-readable before/after).
- `REPAIR_WRITE=1` additionally: verifies the meeting id's rows exist, dumps the full
  original rows (text, timestamps, `token_timestamps`, speaker columns) to a timestamped
  JSON backup next to the DB, requires the app to be closed, then rewrites ONLY the
  replaced rows in one transaction — in BOTH tables once `align-from-immutable-source`
  lands: the repaired text is new transcription output, so it must become the meeting's
  SOURCE rows (`transcript_sources`, keeping the fresh decode's token JSON — real data
  the aligner consumes), and the rendering rows (`transcripts`) get the same text with
  `token_timestamps = NULL` and speaker label cleared — stale labels are worse than
  absent ones, and diarization re-runs afterwards via the existing Speakers/Enhance
  flow, now re-deriving from the healed source. A re-decoded window may yield N
  segments: each becomes a source row (the window's transcription output), and the
  rendering rows covering the window are replaced by the same set. Writing the
  rendering row alone is NOT sufficient post-split: the next Speakers run would
  re-align from the still-garbage source and resurrect the hallucination. No
  speaker-lane code is touched.
- Outcome per flagged row is one of: `repaired-clean`, `dropped` (content-free loops),
  reported with counts and the full dropped text for audit. The harness never leaves a
  flagged row in place silently.

## Sequencing (binding)

ORDERING (policy, not necessity): tasks 4.2/5.2 sequence after
`align-from-immutable-source` lands, because its `transcript_sources` table is the
repair's durable write target and its re-pin regime owns the fixture flow. The reverse
order is also sound (a pre-split repair edits the single table, and the eager backfill
would then freeze the repaired rows durably) — what is NOT sound is a rendering-only
repair executed AFTER the split, which the next Speakers run clobbers; tasks 4.1/4.2
are written dual-table so either ordering works. Tasks 1–3 (detector, language
resolution, batch-loop quarantine) are fully independent of that change: they act on
fresh decodes before the transcription save path, which the source-split turns into an
automatic dual-write.

`no-split-sentences` freezes cde5c264 (no Enhance/Speakers, SHA-pinned snapshot)
until it archives. Task 5.2's repair run on cde5c264 executes only AFTER
`no-split-sentences` archives, followed by re-pinning that snapshot fixture and its
`row_sha256` per the no-split re-pin regime AND regenerating
`tools/expected-flags.json` for the new sha (the fixture gate fails loudly on an
unkeyed sha). Tasks 1–4 and 5.1 have no such dependency.

## Risks

- Legitimate code-switched speech (a real Korean sentence in an English meeting) is
  flagged by design. Retry keeps a pinned-language decode; if the audio truly contains
  Korean, the `en` decode transliterates or drops it — acceptable for this product's
  English-default posture, and the drop path is counted, not silent.
- Detector thresholds are calibrated on one meeting (413 rows). The tracked
  expectations file makes drift loud; thresholds are consts with the fixture numbers in
  their doc comments so recalibration is a visible diff.
- whisper-rs 0.16 exposes no `suppress_regex`; regex-based suppression (e.g. the known
  "Subtitle" hallucination strings) is out of scope for this binding.
