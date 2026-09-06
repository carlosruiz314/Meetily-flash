# Hybrid Diarization Engine

## Why

Speaker attribution on real meetings is pervasively wrong: sentences cut mid-flow across speaker labels the ear says are wrong (user-verified on cde5c264 at 5.9–12.8s and 02:12–02:50s). Session-long diagnostics prove the defects live in the **assembly layer**, not the models: pyannote boundaries land on real pauses; TitaNet embeddings are reliable at ≥1.5s (0.73–0.94 affinity) and separate the meeting's three voices at run level (within 0.488 vs across 0.084 cosine); the glue that assembles turns manufactured the failures. There is also **no automated acceptance gate**: every prior "fix" was graded by the user's ears after the fact, which is the process that produced repeated regressions (misattributed claims, aggregation bugs like a "crosstalk 83%" flag where raw frames showed ≈2.5%, and previews that still showed the flagship defect).

Sequencing: this change depends on `speaker-identity-embedding-stamping` (the stamped-embedding pool its re-diarization semantics rely on) and must archive after `decommission-queue-diarization-phase`, onto whose transport (direct `run_diarization_for_meeting` invocation, no queue phase) the MODIFIED requirement is written; it also carries the reconciliation clause with `sentence-aware-turn-assembly` (its persist-path rules keep governing legacy/fallback rows; the derivation engine owns success-path turn units).

## What Changes

- **Turn units re-based on pyannote per-frame activity**: pause-delimited *speech* runs (speech-vs-silence), then sub-splits where the per-frame label track changes speaker index — a split is admitted only when independently decoded windows agree at the split point (seam permutations rejected).
- **Voice-synchronous labeling**: one TitaNet embedding per labeled piece (≥1.5s, matching `MIN_SPEECH_SECS`; pieces >12s embed their middle 12s, matching the validated measurement); threshold clustering over pieces with the production most-isolated merge-to-cap policy; sub-floor or margin-ambiguous pieces attach to an adjacent turn (previous by default) and can never open a new label — with a 5s cap on contiguous absorbed material: beyond it, a forced separate turn is emitted, flagged low-confidence in the data (an invisible wrong absorption is a defect class too).
- **Textless voiced runs (breaths/laughs/grunts whisper never transcribed) are dropped before coalescing**; min-duration absorption applies to silence and same-label fragments only — a short different-label run becomes a piece and follows the attachment rules.
- **Span-truthful overlap flags**: recomputed from raw per-frame overlap-pair mass over the final merged span (fragment maxima structurally excluded). Persisted in this change; rendering the flag in the UI is a follow-up.
- **Engine-emitted continuation fact**: each persisted turn carries `continues_previous` (same-label adjacency across absorbed silence, or a backward-attached ambiguous piece at turn start). The UI renders the engine fact first; the existing text heuristic (`isContinuation`) becomes fallback only. Requires a small migration (nullable column). **Hard invariant, enforced automatically over the full meeting output: any turn starting mid-sentence (lowercase-initial) must carry the flag — an unmarked one fails the gate and never reaches the user.**
- **Ear-truth fixture gate**: user-confirmed attribution facts for cde5c264 pinned as data (`tests/fixtures/ear_truth_cde5c264.json`); a gate test asserts the engine against every entry. Entries are seeded by the implementer from existing diagnostics and **confirmed or denied by the user per entry** (yes/no, no authoring burden); entries change only with explicit user confirmation. Per-entry outcomes: PASS, or KNOWN-LIMITATION documented with explicit user sign-off, or FAIL (blocks).
- **Re-diarization of manually-labeled meetings defined**: an explicit Speakers re-run re-derives all rows including previously renamed ones; user-applied names re-apply via the stamped-embedding match and unmatched names are reported (requires user sign-off on this semantics during apply — it changes current skip-manual-rows behavior).

User-facing acceptance (the definition of done): on cde5c264, the first 60s renders as a small set of turns (no mid-sentence cuts at the pinned spans), **no turn in the entire meeting starts mid-sentence without the continuation flag (automatically scanned — the user is never shown an unmarked sentence-speaker cut)**, every remaining cross-speaker sentence continuation is marked `…`, the fixture gate is fully PASS or every non-PASS entry is a user-signed KNOWN-LIMITATION, and the frontend transcript suites pass against the new output. Expected user touchpoints: two scheduled — fixture-entry confirmation (which includes sign-off on the re-diarization semantics, see below) and the final rendered-preview review — plus contingent KNOWN-LIMITATION sign-offs only if the gate cannot pass an entry.

Out of scope (follow-on changes): speaker enrollment (name + reference clip → verification-mode attribution); whisper text cleanup; overlap-flag UI rendering.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `speaker-diarization`: turn derivation moves from chunk-grid/embedding labeling to pyannote speech-run assembly with verified sub-run voice-change splits; textless-run handling, span-truthful overlap, engine-emitted continuation facts, re-diarization semantics for manually-labeled meetings, and the ear-truth fixture gate are specified; success-path requirements the engine supersedes are scoped to the fallback path.

## Impact

- **Code**: `frontend/src-tauri/src/audio/speaker/` — new assembly engine consuming per-frame masses from `pyannote_segmentation`; success path invoked directly from `run_diarization_for_meeting` (single pyannote pass; `boundary_segments` call retired on this path); `OrtDiarizationAdapter::process` + chunk grid remain the model-missing fallback; `speaker.rs` alignment/re-diarization changes for manual rows.
- **Data**: one migration adding a nullable `continues_previous` column to `transcripts`; no other schema change; existing consolidated turns re-derive on the next explicit Speakers run (renames re-applied per the semantics above).
- **Tests**: new fixture + gate test (`tests/ear_truth_gate.rs`); synthetic frame-array unit tests (CI-runnable); the live diag harnesses stay as manual probes.
- **UI**: no new components; `VirtualizedTranscriptView` consumes `continues_previous` (engine fact first); verification task covers rendering, rename/revert flows, and long-row behavior.
- **Models**: unchanged (pyannote-segmentation.onnx + nemo-titanet); no new downloads.
- **Runtime**: full-meeting pyannote inference ≈10–12 min (single pass — accepted by the user).
