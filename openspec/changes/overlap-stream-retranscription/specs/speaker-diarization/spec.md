## ADDED Requirements

### Requirement: Overlap spans render per-voice rows from separated streams

When the separation pre-pass identifies an overlap span and the margin-gated
voice votes are decisive for BOTH separated streams (one vote each, at or
above margin), the pipeline SHALL transcribe each separated stream (span
carved with context padding, RMS-normalized) through the loaded Whisper
engine, and the span's mixture rows in the regenerated rendering SHALL be
replaced by one row per decisive stream — span walls, the stream's voice
badge, the stream's text. Each stream row SHALL inherit the span's source
row id as `original_id` so persistence and undo flows can trace it (a
rendering row whose `original_id` is absent from `transcript_sources` is
silently dropped at persist).

The synthesis pass SHALL run after duplicate-cluster resolution — a
same-span stream pair SHALL never be classified as a re-transcription
cluster and dropped. Stream text is untrusted model output: it SHALL pass
the hallucination guards (non-empty, not quarantined, degenerate-repeat and
echo checks) exactly as live transcription output does.

If any condition fails — vote margin miss on either stream, empty or
quarantined stream text, separation model missing, transcription engine
unavailable — the rendering SHALL be byte-identical to today's mixture
render. `transcript_sources` SHALL remain byte-identical across the whole
flow. A word-loss comparison (the synthesized rows' word count vs the
mixture row's) SHALL be computed and surfaced as a diagnostic flag on the
span; the flag SHALL NOT drop or revert any row.

#### Scenario: Decisive overlap span renders two per-voice rows

- **GIVEN** an overlap span where both separated streams have
  margin-passing voice votes and guard-clean text
- **WHEN** the rendering regenerates
- **THEN** the span's mixture rows are replaced by two rows — one per
  stream — with span walls and each stream's voice badge
- **AND** each stream row's `original_id` is the span's source row id

#### Scenario: Undecidable span degrades to the mixture render

- **GIVEN** an overlap span where a vote margin misses, a stream's text is
  empty or quarantined, the separation model is missing, or the
  transcription engine is unavailable
- **WHEN** the rendering regenerates
- **THEN** the span renders exactly today's mixture rows, byte-identical

#### Scenario: Immutability and persist round-trip

- **GIVEN** a full diarization run that synthesized at least one overlap
  span
- **WHEN** the render persists and is refetched
- **THEN** `transcript_sources` is byte-identical to before the run
- **AND** the persisted stream rows keep their per-voice badges and text

#### Scenario: Word-loss flag is diagnostic only

- **GIVEN** a synthesized span whose stream rows contain fewer words than
  the mixture row they replaced
- **WHEN** the render and the gate census are produced
- **THEN** the span is flagged in the census
- **AND** the span still renders its synthesized rows — nothing is dropped
  or reverted by the flag

#### Scenario: Stream pair survives duplicate-cluster resolution

- **GIVEN** a same-span stream pair whose texts are token-similar (an echo
  or a genuine repeat)
- **WHEN** the duplicate-cluster resolver runs
- **THEN** both stream rows survive — no voice vanishes

## MODIFIED Requirements

### Requirement: Ear-truth fixture gate validates attribution

The repository SHALL contain a pinned ear-truth fixture (`frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json`) holding attribution facts as data, each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). The 13 recorded entries (user-ear answers of 2026-09-04, verbatim in this change's `fixture-answers.md`, resolved to absolute times via recovered clip offsets): single-voice spans ≈5.9–12.8 (user's sentence), 15.5–20.8, 24.5–29.5 (UserB), 32.0–38.0, 2803–2820 (UserC); voice changes at ≈13.0 (user→UserB), ≈29.5 (UserB→user), ≈31.5 (user→UserB "Yeah"), ≈38.0/≈39.0 ("okay" interjection), ≈2776.4 (two voices trading), ≈2803.0 (UserB→UserC), ≈2821.0 (UserC→UserB), and the 02:12–02:50s anchor `voice_change_at` ≈161s ±0.75 with the "And I was like, oh, when you put a that one" tail on the earlier side. Entries change only with explicit user confirmation, and two entries (`S3_updates_run`, `S13_userC_to_userB`) SHALL be designated hold-out (not used for any calibration decision).

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file inside the change folder at every verification point, so the acceptance evidence is inspectable without re-running. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented in this change with explicit user sign-off); or FAIL (blocks the change). A synthetic subset of the gate (the frame/split/attachment rules on recorded fixture arrays) SHALL run in plain `cargo test` without audio or models.

The gate's duplicate scan SHALL be provenance-scoped at group level: overlapping-wall turn groups whose absorbed-row id sets intersect (the synthesized stream pair shares the span's source row id — the both-streams-decisive gate is the attestation of simultaneity) SHALL pass the scan as attested simultaneous speech; overlapping-wall groups with disjoint id sets remain double-decode suspects and SHALL still fail the scan. Wall-offsetting (shifting timestamps to dodge the scan) SHALL NOT be used. The gate SHALL log a census of every synthesized span — walls, both voices, per-stream text, the trigger mass and vote margins that fired it, and the word-loss flag — as the ear-calibration inventory for the user's clip-based sampling; the census is diagnostic and never drops rows.

#### Scenario: Fixture gate validates the engine before review

- **GIVEN** the meeting audio, local models, and the gate env set
- **WHEN** the gate test runs
- **THEN** every ear-truth entry passes against the derived turns, or each non-passing entry carries a recorded KNOWN-LIMITATION with user sign-off
- **AND** the recorded output file in the change folder reflects the latest run

#### Scenario: Fixture failure blocks the change

- **GIVEN** an engine change that moves a pinned boundary or flips a pinned label
- **WHEN** the gate test runs
- **THEN** it fails, naming the violated entry
- **AND** the entry may only be resolved by passing the engine or by user-confirmed KNOWN-LIMITATION

#### Scenario: Synthetic gate subset runs in CI

- **GIVEN** a plain `cargo test` without meeting audio or models
- **WHEN** the gate's synthetic subset runs
- **THEN** the run/split/attachment rules are asserted against recorded fixture arrays without env gates

#### Scenario: Provenance-scoped duplicate-scan exemption

- **GIVEN** a render containing a synthesized stream pair at overlapping
  walls whose rows share the span's source row id
- **WHEN** the gate's duplicate scan runs
- **THEN** the pair is not reported as a double-decode suspect
- **AND** an overlapping-wall pair tracing to disjoint source ids is still
  reported and fails the gate
