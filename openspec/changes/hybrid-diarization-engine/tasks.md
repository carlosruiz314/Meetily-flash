## 1. Ear-truth fixture (the acceptance gate comes first)

- [ ] 1.1 Mine candidate fixture spans from existing diagnostics (near-threshold boundaries, index-change sites, dropped-run regions, long-run middles); render each candidate with its exact text and times; present to the user as a confirm/deny list (no authoring burden); record confirmed entries and mark two as hold-out
- [ ] 1.2 RED: after 1.1 sign-off, create `frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json` (kinds: `single_voice`, `voice_change_at`, `distinct_speaker`) and `tests/ear_truth_gate.rs` env-gated test that runs the CURRENT production engine, asserts every entry, AND scans the FULL output for the no-unmarked-mid-sentence-cut invariant (every lowercase-initial turn must carry `continues_previous`) — must FAIL on the current pipeline, proving detection power
- [ ] 1.3 CI-runnable synthetic subset: record the fixture audio arrays (frames around each pinned boundary) as test data; assert the run/split/attachment rules on them in plain `cargo test` (no audio, no models, no env gate)
- [ ] 1.4 Named gate runner script (`run_gate.bat` pattern) that records gate output into `openspec/changes/hybrid-diarization-engine/gate-runs/`; every later verification point appends a timestamped run record

## 2. Frame-level activity source (production code path)

- [x] 2.1 RED→GREEN: expose per-frame speaker/overlap probability masses from `PyannoteSegmentation` (production geometry: 10s/1s windows, last-writer-wins, zero-padded final window) without changing `change_points` behavior — RED state is the new mass-output assertions (run/overlap values on the recorded fixture arrays); existing `change_points` tests stay green
- [x] 2.2 RED→GREEN: pure run derivation on synthetic frame arrays: silence gate 0.5, min run 0.3s absorbing silence/same-label fragments only, different-label short runs retained as pieces, mode filter over the label track
- [x] 2.3 RED→GREEN: cross-window split corroboration on a synthetic two-window fixture (seam permutation rejected; corroborated change split)

## 3. Run assembly engine

- [x] 3.1 RED→GREEN: piece extraction from corroborated splits; pieces ≥1.5s are labeling candidates; >12s pieces slice to middle-12s; sub-1.5s pieces are attachment-only
- [x] 3.2 RED→GREEN: threshold clustering + most-isolated merge-to-cap (production `enforce_max_speakers_cap` policy, cap received from caller) + nearest-centroid refine; deterministic index/time-ordered ties (no HashMap-order effects); CI determinism unit test on synthetic embeddings; phantom-centroid invariant (persisted centroids ⊆ labeled pieces)
- [x] 3.3 RED→GREEN: margin-gated backward attachment (margin vs final centroids, post-refine), sub-floor attachment (previous turn; following turn only at meeting start), 5s contiguous-absorption cap forcing a low-confidence turn
- [x] 3.4 RED→GREEN: textless-run detection (whisper skew tolerance) dropped BEFORE same-cluster coalescing; coalescing across absorbed silence; "Where | is Ricardo" regression test
- [x] 3.5 RED→GREEN: overlap flag recomputed from raw per-frame overlap-pair mass over the FINAL post-merge span; regression test proving a fragment maximum cannot surface as the span flag
- [x] 3.6 RED→GREEN: text alignment — token-timestamped rows split at turn boundaries, token-less rows split proportionally, zero-overlap rows attach nearest-in-time; content-preservation invariant asserted end-to-end (every input row's alphanumeric content appears in exactly one output turn); "And I was like..." tail lands on the earlier turn at the ≈163s fixture boundary

## 4. Pipeline wiring and data

- [ ] 4.1 Success path: `run_diarization_for_meeting` invokes the assembly engine directly with one pyannote pass (frame masses); `boundary_segments`/chunk path NOT invoked on this path; engine returns `DiarizationOutput`-shaped segments plus `continues_previous` and overlap fraction
- [x] 4.2 Fallback path (corrupt model) untouched: existing grid + AHC + smoothing tests still green; model-absent skip behavior test pinned ("speaker models not found", no labels)
- [ ] 4.3 `refine_pass2` on the success path: decide by the pinned rule (keep iff a fixture entry fails without it); record the decision and the gate run in design.md
- [ ] 4.4 Migration: nullable `continues_previous` column on `transcripts` (LF-pinned, checksum-safe); legacy rows render via heuristic fallback (null flag)
- [x] 4.5 RED→GREEN: re-diarization semantics — enumerate pre-run manual labels BEFORE stale-state cleanup; manual rows re-derive (guard relaxed on the explicit re-run path only); names re-apply via stamped-embedding match; unmatched names reported in the run result (survival test with changed cluster count/ordering) — sign-off on this semantics folds into touchpoint 1 (task 1.1)
- [ ] 4.6 UI plumbing + verification: serialize `continues_previous` through the Tauri command layer into `TranscriptSegmentData` and feed the existing `continuesPrevious` prop in `VirtualizedTranscriptView` (engine fact first, heuristic fallback); render the new engine's cde5c264 output; run frontend suites; walk the checklist (longest row ≈160s, inline rename, revert flow, continuation markers from engine facts, pagination totals)

## 5. Verification

- [ ] 5.1 `cargo test --lib` green (all existing suites)
- [ ] 5.2 Recorded gate run: full ear-truth fixture PASS on the new engine, or every non-PASS entry is a KNOWN-LIMITATION with explicit user sign-off; the full-output no-unmarked-mid-sentence-cut scan (every lowercase-initial turn flagged) reports ZERO violations; output recorded under `gate-runs/`
- [ ] 5.3 Full-meeting live run on cde5c264 through the production engine: pinned boundary locations and cluster count (3 under the meeting's override) asserted via the fixture gate; turn count bounded to the 150–260 range (the engine's corroborated splits and textless drops legitimately differ from the sim's 210); overlap flags recomputed per spec (span fraction, not sim's max-of-pieces); the 02:12 region shows the split at ≈163s with the tail on the earlier speaker
- [ ] 5.4 Extract the rendered review artifact for the user's final review (bounded: fixture spans + diff-highlighted boundaries old-vs-new + annotation that text garbage is whisper output, out of scope); annotate `hybrid_transcript_preview.md` at the repo root as a historical pre-fix simulation so stale artifacts cannot circulate as current output — user reviews once
- [ ] 5.5 OpenSpec archive: sync deltas into `openspec/specs/speaker-diarization/spec.md`, applying the retirement/amendment notes (smoothing, granularity, short-chunks, short-speaker-merge scoped to fallback; centroid-storage and token-timestamp-alignment amended; manual-guard scoped; re-transcription re-pointed; headline requirement RENAMED to drop "queue phase"), reconciling with the archived `decommission-queue-diarization-phase` end state (including its stale-state-cleanup and skip clauses), and recording the layering clause with `sentence-aware-turn-assembly` (derivation engine owns success-path turn units; persist-path assembly governs legacy/fallback rows)
