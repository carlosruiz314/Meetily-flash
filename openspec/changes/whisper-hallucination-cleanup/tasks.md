## 1. Detector module

- [x] 1.1 New `src/audio/hallucination.rs`: `audit(text, start_ms, end_ms) -> HallucinationReport` with the calibrated triggers (fffd ≥1; non-Latin script chars ≥3 via the `unicode-script` crate; loop = ratio ≥0.5 AND top-token count ≥10; wps ≥25 on ≥0.8 s rows), thresholds as documented consts
- [x] 1.2 Unit tests for each trigger family (synthetic rows: CJK/Korean/Cyrillic salad, U+FFFD run, "Okay." ×37 loop, 6× "yeah" backchannel must PASS, clean English with é/ñ/em-dash must pass)
- [x] 1.3 Fixture-pinned integration tests: flagged row ids compared against a tracked copy of `tools/expected-flags.json` landed at `frontend/src-tauri/tests/expected-flags.json` (keyed by each fixture's `row_sha256`); skip with a printed notice when the untracked fixture files are absent; mismatch fails loudly. The gate already caught one drift in practice: the current snapshot was re-pinned by the no-split regime (2952af… → af1e24…, 173→182 rows) and expectations were regenerated per the re-pin regime — pre-live pin unchanged (22/240)

## 2. Language resolution

- [x] 2.1 `resolve_batch_language(pref)`: `None | "auto" | "auto-translate"` → `"auto-translate"`; explicit code → that code. Unit test pins the mapping
- [x] 2.2 `lib.rs` retranscription processor: pass `resolve_batch_language(...)` to `start_retranscription` instead of `None`
- [x] 2.3 `whisper_engine.rs` comment-only correction: the `compression_ratio_threshold` availability note stays; append that the repetition gate in this whisper.cpp is `entropy_thold` and the temperature-fallback ladder is active via default `temperature_inc`
- [x] 2.4 `cargo test --lib` green (666 passed; baseline 648)

## 3. Quarantine in the batch loop

- [x] 3.1 In `transcribe_segments_checkpointed`'s transcribe step: audit → on flag, one retry with a DIFFERENT decode (greedy best_of 5, temperature 0, run's concrete language pinned; `SHOULD_YIELD` re-checked before retrying) → clean retry replaces text and its checkpoint; still-flagged retry is dropped (not accumulated, no checkpoint) and counted/logged
- [x] 3.2 Checkpoint-skip branch audits `cp.text` on resume: flagged → same retry rule → replace the checkpoint row's text, or delete the checkpoint row and drop
- [x] 3.3 Circuit breaker: drops > 30 % of the run's segments ⇒ job Failed, nothing persisted; threshold as a logged const
- [x] 3.4 Per-drop log line (meeting, segment timestamps, trigger stats) and a run-summary count

## 4. Offline repair harness

- [ ] 4.1 `tests/hallucination_repair_run.rs` (live_speakers_run pattern): env-provided DB path + meeting folder; audits the meeting's SOURCE rows (`transcript_sources` post-split — the durable truth; rendering-only audits miss source-only garbage; pre-split single table audited as today), mapping flagged rows to rendering rows by span overlap for the report — a fully-absorbed garbage source row has NO rendering counterpart and the report prints "no rendering counterpart" rather than skipping silently; sample-slices flagged windows from a single decode, retries with task 3.1's different-decode rule, prints before/after with per-row outcome (`repaired-clean` / `dropped` + full dropped text); report-only by default
- [ ] 4.2 `REPAIR_WRITE=1`: verifies the meeting's rows, dumps original rows (text, times, token_timestamps, speaker columns) to a timestamped JSON backup beside the DB, requires the app closed, rewrites ONLY replaced rows in one transaction — in BOTH tables (source `transcript_sources` gets the repaired text as new transcription output, KEEPING the fresh decode's token JSON and stamped `source_origin = 'stt'` — it IS fresh STT output; rendering `transcripts` gets the same text, `token_timestamps` NULL, speaker label cleared; a re-decoded window yielding N segments writes N source rows and replaces the window's rendering rows with the same set). Known transient, stated in the report: a rendering row spanning the window AND a neighboring clean source row is deleted wholesale (regenerated rendering rows span source rows post-assembly) — the neighbor's text is transiently absent from the rendering until the follow-up Speakers run re-derives everything; the source copy is never touched. SEQUENCED after `align-from-immutable-source` (see design Sequencing: reverse order also sound, rendering-only-after-split is not). Integration test: repaired row exists in `transcript_sources` AND a subsequent align returns the repaired text. No speaker-lane code touched

> Status (this session): 4.1/4.2 are implemented to the PRE-revision spec — rendering-table audit + dual-table write keyed by row id, with token JSON nulled in both. The revised source-first spec above (source audit, fresh token JSON kept in `transcript_sources`, `source_origin = 'stt'` stamp, N-rows-per-window, absorbed-row mapping) is the remaining work on this harness and is sequenced with `align-from-immutable-source` per the design; boxes stay unticked until that refinement lands.

## 5. Acceptance

- [x] 5.1 Fixture gate: audit flags exactly the tracked expected sets on both fixtures, zero clean-row flags (task 1.3 green with fixtures present)
- [ ] 5.2 Window repair run on cde5c264 (flagged spans only, no full 83-minute re-transcription): ends with zero flagged rows — each former garbage row `repaired-clean` or `dropped`, counts + dropped text reported; report saved to `openspec/exploration/`. SEQUENCING: only after `no-split-sentences` archives AND after `align-from-immutable-source` lands (repair writes the source table — see 4.2); then re-pin the snapshot fixture and its `row_sha256` per the no-split re-pin regime AND regenerate `tools/expected-flags.json` for the new sha (the task-1.3 gate fails loudly on an unkeyed sha — regenerating it is part of this task, not a follow-up). Post-repair, one Speakers run must re-derive the rendering from the healed source without resurrecting any flagged row
- [x] 5.3 Add `frontend/src-tauri/tests/fixtures/cde5c264_transcripts*.json` to `.gitignore` (fixtures stay untracked)
- [x] 5.4 `openspec validate whisper-hallucination-cleanup` green
