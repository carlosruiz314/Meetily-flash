# Design — Hybrid Diarization Engine

## Context

The production pipeline labels whisper-row/chunk-grid fragments with TitaNet embeddings and AHC. Session diagnostics on cde5c264 (committed probes in `tests/pyannote_activity_diag.rs` / `diarization_diag.rs`) established, with numbers:

- pyannote-segmentation-3.0 (in-process `ort`, production boundary source) emits per-frame powerset activity. Its argmax track is locally stable and pause-delimited, but its speaker indices are window-local and carry **no global identity** (within 0.289 vs across 0.254 cosine) — never a label source by itself.
- TitaNet embeddings are reliable at ≥1.5s (clean slices 0.73–0.94 affinity; the 0.9–1.5s band is marginal, so the floor is set at the spec's `MIN_SPEECH_SECS` 1.5s, not a false 1.4s cliff); sub-1s embeddings are noise. Long-run labeling by majority absorbs voice tails (the 02:12–02:50 defect: the user's voice change is at ≈161s per the recorded ear-truth fixture, after the ≈159s pause; the earlier probe estimate ≈163s was 2s off — the ear-truth gate exists precisely for this).
- Run-level clustering separates this meeting's three voices (within 0.488 vs across 0.084 on 24 reliable runs), but the full-meeting greedy pass produced **10 raw clusters at threshold 0.40** — merged to 3 only by the meeting's cap override. Cluster-count claims must be tied to the cap policy, not asserted.
- Failures live in assembly: pause-vs-voice-change mismatch, textless-run dicing, max-of-pieces overlap aggregation ("83%" where raw frames show ≈2.5%), and an engine that cannot express continuation.
- Process failure: the user was the only acceptance test. Repeated regressions followed.

Surviving pieces: pyannote session (geometry + smoothing constants pinned by validated probes — do not re-tune), `NemoEmbeddingExtractor` (192-dim), registry/threshold plumbing, consolidation persistence, the shipped UI continuation heuristic (becomes fallback), and the fallback chunk-grid path in its entirety.

## Goals / Non-Goals

**Goals:**
- Turn units that are voice-synchronous: speech runs, splits corroborated across windows, per-piece labeling in the reliable regime.
- The measured 02:12–02:50 class fixed and locked by the ear-truth fixture gate.
- Attribution changes graded automatically against user-confirmed facts; user touchpoints reduced to two (fixture confirmation, final preview review).
- Span-truthful overlap flags; engine-emitted continuation facts; defined re-diarization semantics for manually-labeled meetings.

**Non-Goals:**
- Speaker enrollment (name + reference clip → verification mode) — follow-on change.
- Perfect attribution during genuine simultaneous speech — flagged honestly instead; overlap flag is persisted-only in this change.
- Whisper text cleanup (word-salad hallucinations, sentence repeats) — separate change; review artifacts must annotate this so attribution is judged, not text quality.
- New models, new downloads, changed pyannote smoothing constants, >3-speaker powerset support.

## Decisions

**D1 — Two-layer derivation: speech runs, then corroborated splits.**
Layer 1: speech-vs-silence runs (speech = summed speaker mass > 0.5, min duration 0.3s; zero-padded final-window decode so the tail <1s is not dropped). Layer 2: a label-track change splits a run only when the windows adjacent to the split independently show the same change in their own local labeling — implemented by keeping per-window decodes at candidate split sites and requiring cross-window corroboration. Rationale: last-writer-wins merging makes seam-time index permutations possible (windows re-label indices freely); per-window corroboration is the only identity-free check that distinguishes a real acoustic change from a seam re-label. A mode filter over the label track (NOT the per-speaker bool median filter, which never sees the argmax label) removes single-frame flicker first. Alternative rejected: deriving runs from the argmax track directly (makes "sub-run splits" a no-op and bakes seam permutations in as boundaries).

**D2 — Piece floors and slices.**
Labeling floor = 1.5s (`MIN_SPEECH_SECS` — resolves the 1.4s contradiction and keeps the model's stated minimum input). Pieces >12s embed their **middle 12s** — this is what the validated numbers (0.488/0.084) actually measured; whole-60–160s embeddings are unvalidated. Sub-floor pieces are embedded ONLY for attachment (cosine), never for labeling. Min-duration collapsing absorbs silence and same-label fragments only; a short different-label run becomes a piece.

**D3 — Attachment is temporal-first, margin-gated, capped.**
Attachment order: previous turn by default; following turn only at meeting start. Ambiguity margin (best vs second final-centroid cosine < 0.05, computed AFTER the refinement pass so refine cannot overwrite attachment) attaches backward. Contiguous backward-attached material >5s forces its own low-confidence turn — unbounded invisible absorption is a defect class (a wrong label at least shows a boundary). On the measured meeting 638 of 1553 runs are sub-floor: attachment is a load-bearing path, hence the fixture's synthetic subset asserts these rules on recorded arrays in CI.

**D4 — Cap and merge policy = production's, exactly one policy.**
Runs are shed to a piece cap (2000) before embedding — positional shed, sub-floor survivors merged within their speech region — bounding clustering (≤2000² cosine work, seconds) regardless of meeting length. Over-cap raw clusters merge via the production **most-isolated** policy (`enforce_max_speakers_cap` semantics, which protects two similar real speakers), not closest-pair fusion; the engine receives the meeting's resolved cap (including the DB-resolved per-meeting override) from `run_diarization_for_meeting` — the same source as today — and the post-cap invariant (persisted centroids ⊆ labeled pieces ⊆ centroid labels; prune moved out of the pass-2 branch) is asserted by test. No claim is made that cluster count equals 3 on meetings without the override; the fixture pins 3 for cde5c264 via its existing override.

**D5 — One pyannote pass; direct invocation on the success path.**
`run_diarization_for_meeting` calls the assembly engine directly with the frame masses from a single full-meeting pass; `PyannoteSegmentation::boundary_segments` and the chunk-grid path are NOT invoked on this path (avoids a doubled ~10-min inference). `OrtDiarizationAdapter::process` + chunk grid + smoothing remain the corrupt-model fallback, behind the same branch. The engine returns the existing `DiarizationOutput`-shaped segments plus per-turn `continues_previous` and overlap fraction, so persistence keeps working.

**D6 — Continuation is an engine fact, not a text guess.**
Each persisted turn carries `continues_previous` (backward-attached ambiguous piece at turn start, or same-label adjacency across absorbed silence ⇒ true; corroborated voice change ⇒ false), migrated in as a nullable `transcripts` column. UI renders the engine fact; the `isContinuation` text heuristic is fallback for legacy rows (null flag). False markers are fixture-gated on `voice_change_at` entries. Rationale: the heuristic mis-fires on whisper's unpunctuated text (77% of rows on the reference meeting) — with this user, a lying marker is as bad as a lying label. Additionally the gate enforces the user's hard rule as a full-output invariant: a turn whose text starts mid-sentence (lowercase-initial after leading-punct strip) without `continues_previous = true` fails the gate — lowercase-initial segments are suspects by default, and the machine does the suspecting so the user never has to.

**D7 — Re-diarization re-derives manual rows; names re-apply via the stamped pool.**
Today `persist_aligned_splits` skips `speaker_source='manual'` rows, freezing renamed text on old boundaries forever (mixed-generation transcripts after any engine change). New semantics: an explicit Speakers re-run re-derives all rows; user names re-apply through the existing stamped-embedding match (`relink`/fingerprint pool); unmatched names are reported in the run result. This changes user-visible rename persistence and therefore requires explicit user sign-off during apply before the flip (it is called out in tasks, and the survival test uses a changed cluster count/ordering).

**D8 — The fixture gate is data, tests, and a recorded runner — not prose.**
Fixture entries are implementer-mined candidate spans (from the existing diagnostics: near-threshold boundaries, index-change sites, dropped-run regions, long-run middles) that the user confirms/denies per entry — authoring, not confirming, is what made the "10 minutes" promise unrealistic. Two entries are hold-out (never used for calibration decisions; guards against grading on the training set). The named runner script records gate output into the change folder at every verification point; the merge-policy language lives in tasks.md, not the spec. Calibration decisions (margin 0.05 default, split thresholds, mode-filter radius) may only tune against non-hold-out entries; a hold-out failure is a KNOWN-LIMITATION candidate requiring user sign-off, never silently tuned away.

**D9 — Persist-layer ownership: the engine's turns are final on the success path.**
Legacy `consolidate_meeting_turns` and the persist-path turn assembly from `sentence-aware-turn-assembly` exist to shape legacy/fallback rows; they SHALL NOT re-run over success-path engine output, whose coalescing already encodes those rules at derivation time. Layering order: derivation engine (this change) is authoritative for turn units; persist-path assembly applies only to fallback-path and legacy data; if any persist merge ever touches engine output, `continues_previous` is re-derived from the merged members. This is the reconciliation clause with the active `sentence-aware-turn-assembly` change (which archives first; its persist rules keep governing legacy rows).

**D10 — Failure branches.** Embedding-model unloadable ⇒ run fails with the error surfaced (no partial labels); pyannote mid-pass failure ⇒ run fails without partial persistence; segmentation model absent ⇒ skip ("speaker models not found"); segmentation model corrupt ⇒ fallback grid path. Enumerated in the spec's fallback scenario.

## Engine calibration record (gate iterations, 2026-09-04)

- **Split corroboration tolerance 0.2s → 0.35s**: per-window decodes jitter
  >0.2s at real changes; the tight window silently swallowed corroborated
  boundaries. Calibrated on non-hold-out entries.
- **Sub-floor arbitration (D3 amendment)**: promotion floor 0.8s (a 0.37s
  fragment once won with margin 0.28 on pure noise); burst-join for <0.8s
  fragments between agreeing neighbors; sandwich rule keeps interjection
  boundaries; undecided ≥-floor pieces keep their boundary instead of being
  absorbed (the 39.0s "okay" was eaten twice by over-eager joining).
- **Marker semantics**: `continues_previous` = engine fact OR text begins
  mid-sentence (`effective_continuation`). A voice change cutting a shared
  whisper row mid-sentence is legitimately marked; a marker on a fresh
  sentence at a pinned voice change remains a defect.
- **Known model gaps pending user sign-off (gate FAILs by design)**: S5/S6 —
  every per-window decode renders the ≈26.4–34.66s stretch as continuous
  single-speaker speech, so the user-heard changes at ≈30.0/≈32.65 have NO
  model signal for any assembly logic to find; S7 — the 8-window-corroborated
  change at 34.66s plus confident TitaNet separation (margin 0.157–0.40)
  contradict the ear's "same voice through 32–38". Named next lever for
  S5/S6: mixed-piece sub-window scanning (TitaNet change detection inside
  ambiguous long pieces).

## Risks / Trade-offs (continued)

- [Meetings with >3 simultaneous voices: powerset segmentation degrades] → Explicit non-goal; label count still follows clustering + cap (identity comes from TitaNet, not the powerset indices); noted so the degradation is expected, not discovered.
- [Shedding past 2000 pieces is permanent (no pass-2 re-labeling on this path)] → Accepted: ~2h+ meetings only; attribution lost beyond the cap; documented in the spec's cap clause.
- [Token-less consolidated rows split proportionally at boundaries (word-to-time error on long rows)] → Accepted limitation; re-transcription restores token-level splits; fixture gate tests the engine on raw audio.

## Risks / Trade-offs

- [Full-meeting pyannote inference ≈10–12 min per Speakers run] → Single pass (D5) — already the cost of today's boundary pass; accepted by the user explicitly.
- [Seam-permutation false splits despite corroboration] → Cross-window corroboration (D1) + synthetic two-window CI test + fixture gate; residual risk logged if the gate trips.
- [Sub-floor attachment mis-attaches backchannels] → Temporal-first rule, 5s absorption cap, low-confidence flag on forced turns; fixture `voice_change_at` entries pin the tail-side behavior.
- [Long monologue turns (measured: median 12.3s, p90 ≈60s, max ≈162s in the sim)] → Text rows preserve intra-turn sentence flow; UI verification task walks virtualization/rename/revert on the longest rows before ship.
- [10 raw clusters on meetings without a cap override] → Existing production behavior (default cap); unchanged by this change; the most-isolated policy and stamped-pool pruning bounds the damage identically to today. Engine does not invent a new default.
- [Old-engine meetings re-diarize onto consolidated rows without token timestamps (proportional text split accuracy)] → Acknowledged limitation; re-transcription restores granularity; fixture gate tests the engine on raw audio and persistence keeps the content-preservation invariant.
- [Calibration on cde5c264 alone] → Hold-out entries (D8) + synthetic CI subset + append-only fixture design for future meetings.
- [Silence-only / degenerate meetings] → Zero labeled pieces ⇒ zero labels, no error (spec scenario); the sim's `centroids[0]` panic pattern must not survive into the engine.

## Migration Plan

1. Fixture + gate first (task group 1): user confirms entries; gate runs RED against the current engine (proving detection power); synthetic subset green in CI.
2. Build the assembly engine beside `sherpa_adapter`; flip the success path only when the recorded gate output is green or user-signed KNOWN-LIMITATION.
3. Fallback path untouched throughout; `refine_pass2` retirement on the success path is decided by a pinned rule: keep iff some fixture entry fails without it (record the outcome in this design file).
4. `continues_previous` migration ships with the flip; legacy rows render via the heuristic fallback (null flag).
5. Rollback = revert the success-path flip; schema addition is nullable and backward-compatible; existing turns re-derive on the next explicit Speakers run (manual rows included, per D7 sign-off).

## Open Questions

(None carried into implementation — former open questions are pinned:)
- Ambiguity margin / split thresholds: tunable ONLY against non-hold-out fixture entries; any change recorded with the gate output.
- `refine_pass2` on the success path: keep iff a fixture entry fails without it (decide at task 4.3 with the recorded gate run).
- Centroid pool on the success path: run/piece centroids, stamped-pool semantics unchanged, phantom-prune invariant tested (task 3.2).
