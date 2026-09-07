## ADDED Requirements

### Requirement: Speech in pyannote-silence gaps is attributed by voice

On the success path, after clustering and centroid finalization (post refinement and dedup), the engine SHALL rescue speech that pyannote decoded as silence: for every silence gap (the complement of the speech runs; zero-length leading/trailing gaps are skipped, and silences shorter than the run-absorption minimum never form gaps) that overlaps a transcript text span, the engine SHALL embed the gap audio restricted to the span's intersection with the gap using the production nemo_titanet extractor and compare the embedding against the meeting's final centroids via the same nearest-centroid machinery used for sub-floor attachment. When several transcript rows intersect one gap, the span SHALL be the UNION of the intersecting row intervals clipped to the gap — one candidate and one rescue embedding per gap. The embed window SHALL reuse the production slice convention (`embed_slice`: middle-12s for windows longer than 12s) so rescue embeddings are computed identically to the centroid-building embeddings. Transcript text spans are an APPROXIMATION of the render layer's alignable fragments (this meeting predates token timestamps, where rows and fragments coincide); the engine SHALL NOT use text content, only span geometry.

A candidate SHALL be selected only when ALL of the following hold:

1. The extractor returns an embedding. The extractor's silence gate is digital-silence-only (mean-square < 1e-10), so it guards against Whisper text over exact digital silence, not over room tone or breath — the margin gate (3) is the substantive protection against voiced non-speech. Any per-gap extraction failure SHALL abstain (no splice, no error); run-fatal embedding failure remains model-load failure only, unchanged.
2. The span's intersection with the gap is at least the promotion floor (0.8s — the same floor whose calibration established that shorter slices produce confidently-wrong identities on this extractor), measured on the RAW span ∩ gap; the floor guards the TEXT evidence and makes a rescued candidate a NATIVE promotion case under the run-assembly requirement's sub-floor rules. This floor is calibratable only under the ear-truth fixture gate's governance ("Ear-truth fixture gate validates attribution": hold-out entries excluded from calibration decisions; recalibration SHALL leave every entry passing or carrying a user-confirmed amendment), and never below 0.8s without probe-measured false-decision rates on silence-gap audio.
3. The identity is decided: the similarity margin between best and second-best centroid is at or above the rescue margin (default: the ambiguity margin, 0.05), evaluated against the meeting's final centroid set. A single-cluster meeting SHALL abstain (no margin is defined). A rescue-specific margin or an additional absolute best-cosine floor MAY be calibrated only under the same governance, informed by probe-measured inter-centroid geometry.
4. The gap separates two DISTINCT turns. Turn membership at a gap edge is by ADJACENCY — the nearest turn ending at/before the left edge and the nearest turn starting at/after the right edge (strict containment would leave a gap edge that coincides with a turn boundary in no turn). A gap is INTERIOR when both edges are adjacent to the same turn, and interior gaps SHALL abstain: splicing inside a turn's span manufactures an enter-and-exit boundary pair inside attributed speech, the most regression-prone class, with no attribution error to fix (the turn already carries one label). Meeting-edge gaps (at most one flanking turn) are out of scope: they present no distinct-turn pair and are not candidates.
5. The best cluster differs from the candidate's borrow winner — a deterministic engine-side geometric rule modeling what the render layer's gap borrow would do: reference point = the midpoint of span ∩ gap, distance = point-to-turn-span (containment = 0, start-inclusive/end-exclusive), integer-millisecond arithmetic, ties broken by the earlier turn (the render layer's stable time-order behavior). A turn farther than the borrow cap (GAP_BORROW_MAX_MS) from the reference point is not a winner; when NEITHER flank is within the cap there is no winner and the candidate SHALL splice when gates 1–4 hold AND the gap's last decided voiced sub-window exists — decided voice evidence replaces geometry that would leave the row unattributed. (The modeled winner is an approximation used solely for contradiction-gating: the render borrow itself fires only on Unknown fragments — fine transcript rows that overlap no turn; the coarse mega-rows this meeting's gate source uses never render as Unknown, and there the rescue-created boundary is what places proportionally-allocated text correctly.)

Splice placement and identity SHALL come from ENERGY-SEGMENTED VOICED SUB-WINDOWS, not the raw span and not a single first onset: the engine SHALL segment span ∩ gap by signal energy (frames above a robust baseline of the gap's own non-voiced level, sustained for a minimum voiced run) into voiced sub-windows (each at least a minimum sub-window duration, default 0.25s; calibratable only under the ear-truth fixture gate's governance, like every other engine tunable), embed and decide EACH sub-window against the meeting's final centroids, and attribute the transcript row to the LAST decided sub-window in the gap — legacy rows skew early (ear-attested ≈1.0s on cde5c264), so the row's words sit at its tail. A gap may hold more than one voice (probe-attested on cde5c264: Carlos's tail at 14.78, Cynthia at 15.8); an earlier sub-window whose decided identity equals its adjacent flank is a same-voice continuation and produces no piece (today's behavior is already correct there), and this requirement scopes attribution to the last decided sub-window only — earlier disagreeing sub-windows are diagnostics, never splices. If no sub-window is decided, the candidate SHALL abstain. The spliced piece spans exactly the attributing sub-window (a sub-floor span is legitimate — the piece carries `promoted_subfloor = true`), and the embedding window IS the sub-window (gate 2's floor guards the raw span's text evidence; a sub-window MUST NOT be padded to satisfy it).

Candidate evaluation SHALL be a snapshot: all candidates are computed once against the PRE-rescue turn set (the resolve_turns output over the un-augmented piece list), then all selected candidates are spliced in one time-ordered pass as synthetic pieces (`cluster = best`, `margin`, `promoted_subfloor = true`, span = the attributing sub-window), and resolve_turns runs again to produce the final turn set. Because the synthetic piece carries `promoted_subfloor = true` with a decided cluster — the same representation as a natively promoted piece — it plays the existing resolve_turns rules with no special-casing, even though the spliced span itself may be far below the promotion floor (gate 2 measures the floor on the RAW span ∩ gap, guarding the text evidence; the flag, not duration, drives the promotion path). The rules it plays: a candidate matching the far flank's cluster coalesces into it (that flank's turn extends across the gap — the surviving turn carries the promotion flag when the candidate founds the turn, and keeps its own flags when the candidate coalesces into it), a candidate differing from both flanks stands as its own low-confidence turn, and `continues_previous` derives per "Persisted turns carry an engine-emitted continuation fact" unchanged. Candidate selection SHALL be a pure, deterministic function (time/index-ordered decisions only; embeddings passed as an ordered slice parallel to the ordered candidate list — no hash-map iteration in the decision path). The number of rescue embeddings SHALL be bounded by the count of text-bearing distinct-turn gaps.

**Amendment cross-references:** the transcript-row doctrine this requirement extends is invariant 2 of "Transcript-timestamp-driven speaker diarization runs as a post-processing queue phase" (see the MODIFIED block in this delta); the turn-formation layers it extends are "Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits" (same).

#### Scenario: Gap speech attributed via the last voiced sub-window (S2b)

- **GIVEN** the cde5c264 fixture entry S2b (user-ear-recorded 2026-09-07 via clip_E: Cynthia's "Oh, man" starts ≈15.8s; pin change_at 15.8 ±0.75), where pyannote decodes 14.78–16.12s as silence, the engine's existing 16.12s boundary ALREADY passes the entry, and the gap holds TWO voices (probe-attested): Carlos's speech tail from 14.78 and Cynthia from ≈15.8
- **WHEN** the engine derives turns with the rescue pass
- **THEN** the raw span ∩ gap (1.06–1.34s by source) clears the 0.8s floor; energy segmentation yields the Carlos sub-window (identity sp0 = left flank → same-voice continuation, no piece) and the Cynthia sub-window [≈15.8, 16.12] (decided sp1, margin 0.096)
- **AND** the row is attributed to the last decided sub-window (Cynthia), which differs from the borrow winner (sp0 via the row midpoint) — the splice [≈15.8, 16.12] coalesces into the right (Cynthia) turn, moving the boundary from 16.12 to ≈15.8
- **AND** S2b still passes (improving from |0.32| to |0.00|), the "Oh, man" text renders under Cynthia, and no entry that passed before the rescue regresses

#### Scenario: A skewed row start never becomes a boundary

- **GIVEN** a text-bearing gap whose transcript row starts well before any voiced energy (legacy row-timestamp skew, ear-attested ≈1.0s on cde5c264)
- **WHEN** the rescue pass runs
- **THEN** the spliced piece spans the attributing voiced sub-window, never the raw row start
- **AND** if no voiced sub-window is decided, the candidate abstains entirely (no boundary is manufactured from a skewed timestamp)

#### Scenario: A gap interior to one turn abstains (the S7 class)

- **GIVEN** any silence gap that forms interior to a coalesced turn's span (e.g. separated from its neighbors by a dropped textless run or back-attached material — the run-absorption minimum means most interior silences never form gaps at all), where the covering row text mixes a short backchannel with the turn speaker's speech
- **WHEN** the rescue pass runs
- **THEN** no candidate is selected regardless of the embedding's margin, because the gap does not separate two distinct turns
- **AND** no enter-and-exit boundary pair is manufactured inside attributed speech (entries S6, S7, S7b are undisturbed by construction)

#### Scenario: Multi-row gaps yield one candidate

- **GIVEN** a text-bearing distinct-turn gap intersecting two transcript rows
- **WHEN** the rescue pass runs
- **THEN** the span is the union of both row intervals clipped to the gap — exactly one candidate and one rescue embedding for the gap
- **AND** the spliced turn set contains no overlapping turn spans

#### Scenario: Sub-floor intersections abstain

- **GIVEN** a text-bearing gap between two distinct turns whose span ∩ gap is below the 0.8s floor
- **WHEN** the rescue pass runs
- **THEN** no candidate is selected (sub-0.8s slices are the measured confidently-wrong regime on this extractor)

#### Scenario: Voice matching the far flank extends that flank

- **GIVEN** a text-bearing gap between a user turn (left) and a Cynthia turn (right), where the row would borrow LEFTWARD (nearer the left edge, or at the tie) but the gap audio embeds to a decided Cynthia identity
- **WHEN** the rescue pass runs
- **THEN** the candidate splices and coalesces into the right (Cynthia) turn, extending it over the gap
- **AND** the row aligns directly against the extended turn — attribution fixed without a same-label turn split

#### Scenario: Decided identity equal to the borrow winner is a no-op

- **GIVEN** a text-bearing distinct-turn gap whose decided best cluster equals the label of the turn the render borrow would give the row
- **WHEN** the rescue pass runs
- **THEN** no piece splices (the pause is already attributed to the correct speaker, and a same-voice pause must not grow turns)

#### Scenario: Decided voice beyond the borrow cap replaces an unattributed row

- **GIVEN** a text-bearing distinct-turn gap whose span ∩ gap midpoint is farther than GAP_BORROW_MAX_MS from both flanking turns, whose last voiced sub-window embeds to a decided identity
- **WHEN** the rescue pass runs
- **THEN** the candidate splices (neither flank is a borrow winner; without rescue, fine transcript rows covering the gap would render Unknown and carry no attribution)

#### Scenario: Undecided identity keeps the status quo

- **GIVEN** a text-bearing gap whose embedding's best/second-best margin is below the rescue margin (crosstalk mixture, marginal slice)
- **WHEN** the rescue pass runs
- **THEN** no piece splices and the row falls through to the geometric borrow (no new boundary from undecided voice evidence)

#### Scenario: Rescue pass is regression-gated end-to-end

- **GIVEN** the full ear-truth fixture gate on real meeting audio (all entries, plus the render-layer assertions)
- **WHEN** the engine runs with the rescue pass enabled
- **THEN** the gate reports zero FAIL outcomes — every entry is PASS or a recorded, user-confirmed amendment/KNOWN-LIMITATION per "Ear-truth fixture gate validates attribution" (on cde5c264 the known-limitation set is the S2 @12.0s sub-run-change miss, user-confirmed 2026-09-07 via clip_D, and entries that pass without the rescue — S2b among them — MUST still pass with it)
- **AND** the render gate stays clean: no Unknown within the borrow cap of a turn edge, zero zero-duration rows, zero unmerged same-label fragment pairs

## MODIFIED Requirements

### Requirement: Transcript-timestamp-driven speaker diarization runs as a post-processing queue phase

**Amendment (sequenced after `decommission-queue-diarization-phase`):** diarization is not a transcription-queue phase; it runs as a direct post-transcription step invoked via `run_diarization_for_meeting` (import, re-transcription, and explicit Speakers re-run call sites). After the transcription (and, where configured, summarisation) steps complete for a meeting, the system SHALL run offline speaker diarization on the meeting's `audio.mp4`:

1. Decode the audio to 16kHz mono f32 samples via `DecodedAudio::to_whisper_format()` (decode SHALL run inside the same blocking closure as the rest of the pipeline)
2. Transcript rows are used for text alignment, textless-run detection, and locating text-bearing silence gaps for the voice-attribution rescue layer — speech regions and turn boundaries otherwise derive from the pyannote per-frame activity per the run-assembly requirement; text content is never a boundary source or a label source
3. On the success path (pyannote segmentation model present), turn boundaries and labels derive from the run-assembly engine ("Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits") using a SINGLE pyannote inference pass; whisper-row/chunk-grid fragments are not the labeling unit (the sole exception is the rescue layer, which derives candidate SPANS from transcript-row geometry while every label comes from voice evidence) and the effective-split grid is not a boundary source. `build_chunks`, the effective-split grid, temporal-coherence smoothing, and per-chunk clustering remain the pyannote-model-missing/corrupt fallback path, unchanged
4. Extract ONE speaker embedding per labeled piece via `SpeakerEmbeddingExtractor` (nemo_titanet; middle-12s slice for pieces >12s); pieces below the 1.5s floor are embedded only for attachment
5. Cluster the labeled pieces (threshold clustering at the configured merge threshold, most-isolated merge to the meeting's max_speakers cap, nearest-centroid refinement); clustering SHALL run off the async executor (on a blocking thread), SHALL be deterministic (ties by index/time order), and SHALL be bounded by the piece shed-to-cap so it completes in bounded wall-clock time for any meeting length
6. Attach unlabeled/ambiguous pieces per the run-assembly requirement
7. Align transcript rows with the derived turns (max-overlap assignment; content-preservation invariant) and persist per the existing `transcripts` schema plus the `continues_previous` fact

Diarization SHALL be skipped if no `audio.mp4` exists (e.g., `auto_save = false`). Imported audio uses the same pipeline. When the segmentation model file is absent, the run reports "speaker models not found" and skips (no labels are produced); when the model file is present but fails to load, the fallback chunk-grid path runs.

#### Scenario: Diarization runs after transcription

- **WHEN** the post-transcription pipeline reaches diarization (after summarisation where configured)
- **THEN** diarization begins on the meeting's `audio.mp4` via `run_diarization_for_meeting`
- **AND** progress is observable through the existing run/result reporting

#### Scenario: Diarization is skipped when no audio file exists

- **WHEN** the pipeline reaches diarization AND the meeting has no `audio.mp4` (e.g., `auto_save = false`)
- **THEN** the diarization step is skipped with no error surfaced

#### Scenario: Diarization runs on imported audio

- **WHEN** an audio file is imported as a new meeting AND the import triggers transcription
- **THEN** diarization produces speaker labels for the imported audio via the same pipeline

#### Scenario: Short meeting derives turns from pyannote speech runs

- **GIVEN** a meeting with ~10 minutes of speech AND the pyannote segmentation model is present on disk
- **WHEN** diarization runs
- **THEN** turn units are pause-delimited speech runs from the pyannote per-frame activity (NOT a fixed-grid chunking)
- **AND** the effective-split grid is not applied on this path

#### Scenario: Model absent skips; model corrupt falls back

- **GIVEN** the pyannote segmentation model file is absent from disk
- **WHEN** diarization runs
- **THEN** the run reports "speaker models not found" and produces no labels
- **GIVEN** instead that the model file is present but fails to load
- **WHEN** diarization runs
- **THEN** the fallback chunk-grid path (`build_chunks` effective-split grid, per-chunk embedding clustering, temporal-coherence smoothing) runs as before and produces labels
- **AND** no panic propagates to the user-facing flow

#### Scenario: Piece clustering completes in bounded time and never freezes the UI

- **GIVEN** an ~83-minute meeting bounded by the piece shed-to-cap (2000 embedded pieces)
- **WHEN** diarization runs
- **THEN** clustering completes in bounded wall-clock time (seconds, not minutes) under the stated bound
- **AND** the async runtime and UI remain responsive because decode/inference/clustering run on a blocking thread

#### Scenario: Clustering is deterministic

- **GIVEN** the same piece embeddings and the same thresholds
- **WHEN** clustering runs twice
- **THEN** labels and centroids are identical, with ties broken by index/time order

#### Scenario: Text rows locate gap speech without sourcing labels

- **GIVEN** a transcript row overlapping a pyannote-decoded silence gap between two distinct turns
- **WHEN** the rescue layer evaluates the gap
- **THEN** the row's span geometry selects the embed window, and the candidate's label comes exclusively from voice similarity against the meeting's centroids
- **AND** a gap with no overlapping row is never embedded

---

**Retirements introduced by this change (with the run-assembly engine on the success path; all retained on the fallback path):**

- Cached-similarity clustering internals and their behavior-identity scenario: retired with the per-chunk clustering on the success path. **Migration**: none — implementation detail of the fallback path, where the mandate and its property test remain in force.
- "Long meeting cap is enforced by pyannote-boundary shedding": superseded on the success path by the piece shed-to-cap (2000). **Migration**: `MAX_DIARIZATION_CHUNKS`/`shed_boundaries_to_cap` continue to govern the fallback path.
- "Merge short-duration speakers into their cosine-nearest larger cluster" (queue-phase item 6): superseded on the success path by the attachment rules. **Migration**: retained verbatim on the fallback path.
- `phase = "diarizing"` queue assertions: superseded by `decommission-queue-diarization-phase` transport. **Migration**: progress observable through `run_diarization_for_meeting` reporting. Two clauses of decommission's end state are carried forward unchanged by this delta's re-write and shall not be lost at archive: stale auto speaker labels/embeddings are cleared before new labels are written, and skipped runs leave existing labels untouched (except where the re-diarization requirement above explicitly changes manual-row handling on the explicit re-run path).
- **Amendments to untouched requirements (success-path scope)**: "Temporal-coherence smoothing prevents clustering contamination and per-chunk flicker" (canonical ~line 690), "Diarization segment granularity resolves speaker turns within Whisper segments" (~line 784), "Short chunks are not attributed to temporally-absent speakers" (~line 867), and "Short-duration noise speakers are merged into nearest cluster" (~line 93, whose `MIN_CLUSTER_FRAC` merge is superseded by the attachment rules) henceforth govern ONLY the fallback path; "Centroid embeddings are stored per speaker per meeting" (~line 250) is amended so that on the success path stored centroids derive from run/piece embeddings and contain no smoothing-refinement clause; "Token-level timestamps align transcript text with diarization speaker boundaries" (~line 108) governs token-timestamped rows and is amended so that token-less rows split proportionally at turn boundaries and the manual-row guard is scoped per the re-diarization requirement above; "Re-transcription clears and re-enqueues diarization" (~line 491) is re-pointed to the `run_diarization_for_meeting` transport. At archive these requirement blocks are updated with this scoping, and the headline requirement is RENAMED to drop the stale "queue phase" phrasing.

### Requirement: Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits

On the success path (pyannote segmentation model present), the system SHALL derive speaker turns in two layers:

1. **Speech runs**: slide the production 10s/1s pyannote window grid over the full meeting (the final <1s tail SHALL be decoded with zero-padding so trailing speech is not silently dropped) and decode per-frame speaker-probability masses from the powerset output; collapse the speech-vs-silence track (speech = summed speaker mass > 0.5) into runs with minimum duration 0.3s. Min-duration collapsing SHALL absorb silence runs and same-label fragments only; a short run with a DIFFERENT label than its neighbor SHALL be retained as a piece and resolved by the attachment rules below.
2. **Sub-run splits**: a run containing a label-track change is split at the change point only where the change is corroborated: the two windows adjacent to the split time, each decoded independently, MUST each show a label-change event at the same time (agreement on the change EVENT and its timestamp within a ±0.35s tolerance — calibrated: per-window decodes jitter more than 0.2s at real changes — window-local index identities are permutation-ambiguous and are never compared across windows). Windows whose decodes disagree at the split point are treated as seam permutations and rejected; the run stays whole. A mode filter over the label track (not the per-speaker bool median filter, which does not apply to argmax labels; radius 3 frames) removes single-frame label flicker before split detection.
3. **Labeling**: each piece of duration ≥1.5s (the `MIN_SPEECH_SECS` floor; the model's minimum embedding input) SHALL be embedded — pieces longer than 12s embed their middle 12s, matching the validated measurement — and clustered by threshold clustering at the configured merge threshold with deterministic tie-breaking. Cluster count SHALL be capped by the meeting's max_speakers cap using the production most-isolated-cluster merge policy (`enforce_max_speakers_cap` semantics), and a nearest-centroid refinement pass SHALL reassign every labeled piece to its final centroids. Every persisted centroid SHALL correspond to at least one persisted labeled piece (no phantom speakers).
4. **Attachment**: a piece below the 1.5s floor MAY be embedded solely for attachment and SHALL attach to the temporally PREVIOUS labeled piece's turn (the following turn only when no previous exists); a piece whose final-centroid similarity margin between best and second-best is below a positive ambiguity margin (default 0.05, computed after the refinement pass) SHALL attach backward the same way. Attachment SHALL never create a new label, and contiguous backward-attached material exceeding 5s within one turn SHALL instead form its own turn flagged low-confidence in the data. Sub-floor arbitration is fixture-calibrated (design D3 amendment): a sub-floor piece at/above a promotion floor (0.8s; shorter slices win on noise) with a decided best cluster (margin ≥ the ambiguity margin) forms its own low-confidence turn; a sub-floor piece whose neighbors AGREE on one cluster joins them (burst fragmentation of one speaker's speech); a sub-floor piece sandwiched between two DIFFERENT clusters keeps the boundary as its own low-confidence turn with a best-effort label — a real interjection the 1.5s floor must not silently absorb.
5. **Textless runs**: runs with no transcript-text overlap (breaths, laughs, untranscribed voiced noise; detected with a whisper-timestamp skew tolerance) SHALL be dropped BEFORE same-cluster coalescing, so a textless run can never split one speaker's stretch into fragments.
6. **Coalescing and text alignment**: same-cluster neighbors separated only by dropped runs or absorbed silence SHALL coalesce, and the engine's turns ARE the persisted turn units on the success path — post-persistence turn merging layers (legacy consolidation, persist-path assembly) SHALL NOT re-run over engine output; where such a merge ever applies, `continues_previous` is re-derived from the merged members (first member's flag). Every transcript text row SHALL land in exactly one turn. Rows with token timestamps SHALL split at turn boundaries (token-level alignment); rows without token timestamps (legacy consolidated rows) SHALL split proportionally at any turn boundary falling inside them; a row with zero time overlap with every turn (a shed span or an abstaining silence gap) SHALL attach to the nearest-in-time turn. This content-preservation invariant SHALL be asserted end-to-end (every input row's alphanumeric content appears in the persisted output).
7. **Overlap flags**: a turn's crosstalk flag SHALL be the fraction of its FINAL merged span whose per-frame overlap-pair probability mass (sum of powerset classes 4–6) exceeds 0.25, recomputed after all merging; fragment-maximum aggregation is forbidden. Overlap flags are persisted but not rendered by this change.
8. **Determinism**: all label decisions SHALL be computed from time-ordered or index-ordered sequences (ordered containers; no HashMap-iteration-order effect on labels, boundaries, or flags), so identical audio, models, and settings on the same binary produce identical turns. The clustering step itself SHALL be covered by a CI-runnable determinism unit test on synthetic embeddings.
9. **Gap rescue**: after centroid finalization, text-bearing silence gaps that separate two distinct turns and satisfy the selection gates of "Speech in pyannote-silence gaps is attributed by voice" SHALL splice synthetic promotion-floor pieces into the piece list before the final turn resolution — the only layer whose candidate SPANS derive from transcript-row geometry, with labels exclusively from voice evidence. Rescue embeddings are bounded by the text-bearing distinct-turn gap count and sit OUTSIDE the runs shed-to-cap of the bound clause below (they cannot inflate clustering cost: they do not participate in clustering, only in final resolution).

Where this requirement and the chunk-grid labeling requirements conflict on the success path, this requirement governs; the pyannote-model-missing fallback path is unchanged. The number of embedded pieces per meeting SHALL be bounded by a shed-to-cap applied to runs before embedding (cap 2000, shed by position, merging sub-floor survivors within same-label spans of their speech region — never across a corroborated voice change), so clustering cost is bounded regardless of meeting length — with the rescue layer's gap-count bound as the sole sanctioned addition. Shedding is permanent on this path (no pass-2 re-labeling exists); spans lost to the cap lose attribution. If the embedding model fails to load, the run SHALL fail with the error surfaced (no partial labels); if pyannote inference fails mid-pass, the run SHALL fail without persisting partial labels. The label-track mode filter SHALL use radius 3 frames (≈50ms), calibratable only under the fixture-gate rule.

#### Scenario: One voice across a mid-sentence pause stays one turn

- **GIVEN** the cde5c264 fixture entry S1 (`[9.38, 11.8]` single_voice — Cynthia's "…five years" stretch; the Cynthia→Carlos change at ≈12.0s beyond it is the recorded S2 KNOWN-LIMITATION, user-ear-confirmed 2026-09-07 via clip_D)
- **WHEN** the engine derives turns
- **THEN** no turn boundary is manufactured inside 9.38–11.8 (in particular not at an intra-sentence whisper-row edge such as 12.07s' neighbors)
- **AND** the turn covering the span carries the same label as the speaker's other clean runs
- **AND** the recorded ≈12.0s miss remains the S2 entry's business (expected-fail), not this scenario's

#### Scenario: Voice change inside a run splits at the verified change point

- **GIVEN** the cde5c264 fixture entry of one voice change in 02:12–02:50s located at ≈161s (±1.0s per the fixture pin, user-ear-recorded 2026-09-04)
- **WHEN** the engine derives turns
- **THEN** the run splits at the corroborated change point, not at the ≈159s pause
- **AND** the sentence tail ("And I was like, oh, when you put a that one") is attributed to the earlier speaker
- **AND** a piece whose final-centroid margin is below the ambiguity margin attaches to the previous turn instead of opening a new one

#### Scenario: A window-seam permutation is rejected as a split

- **GIVEN** a synthetic two-window fixture where the per-frame label indices permute across the 1s window seam but each window's local track is internally consistent
- **WHEN** split candidates are evaluated
- **THEN** the seam-time change is rejected (adjacent-window corroboration fails) and the run stays whole

#### Scenario: Textless runs cannot dice a turn

- **GIVEN** a speaker's continuous speech containing sub-second voiced runs that whisper never transcribed
- **WHEN** turns are assembled
- **THEN** the textless runs are dropped before coalescing
- **AND** the surrounding same-speaker text renders as ONE turn (the "Where | is Ricardo" regression)

#### Scenario: Every transcript row survives into the output

- **GIVEN** any diarization run on a meeting with transcript rows
- **WHEN** turns are assembled and persisted
- **THEN** every input row's alphanumeric content appears in exactly one output turn
- **AND** a token-timestamped row straddling a turn boundary splits at that boundary; a token-less row splits proportionally; a zero-overlap row attaches to the nearest-in-time turn

#### Scenario: Corroborated text tail lands on the earlier turn

- **GIVEN** the 02:12–02:50 fixture entry, where the "And I was like, oh, when you put a that one" tail shares a whisper row with later-speaker text
- **WHEN** the rows are aligned to the derived turns
- **THEN** the row splits at the ≈161s boundary and the tail text is attributed to the earlier turn (whole-row assignment would make this entry structurally unpassable)

#### Scenario: Overlap flag is span-truthful

- **GIVEN** a merged turn whose final span has raw-frame overlap mass above 0.25 on ~2.5% of frames
- **WHEN** the crosstalk flag is computed
- **THEN** the flag reports the span fraction within ±2 percentage points, not the maximum of any pre-merge fragment

#### Scenario: Edge regimes are handled

- **GIVEN** a meeting whose final second contains speech, and a meeting with no embeddable speech runs at all
- **WHEN** the engine runs
- **THEN** trailing speech receives frames via zero-padded final-window decode
- **AND** a meeting with zero labeled pieces produces zero speaker labels and no error

#### Scenario: Deterministic derivation

- **WHEN** the clustering and assembly functions run twice on identical inputs
- **THEN** labels, boundaries, and flags are identical (CI-runnable unit test on synthetic data)
- **AND** all tie-breaks resolve by index/time order, never by hash-map iteration order

#### Scenario: Rescue candidates are snapshot-deterministic

- **GIVEN** two text-bearing gaps in one meeting whose splices would, if evaluated incrementally, change each other's flanking turns
- **WHEN** the rescue pass runs
- **THEN** every candidate is evaluated against the pre-rescue turn set and spliced in one time-ordered pass
- **AND** running the engine twice on identical inputs yields identical turn sets

### Requirement: Ear-truth fixture gate validates attribution

The repository SHALL contain a pinned ear-truth fixture (`frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json`) holding attribution facts as data — the fixture file is the single source of truth for the entry set (16 entries as of 2026-09-07; this requirement does not enumerate them) — each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label, where an overlap is a turn-span intersection exceeding 0.25s, matching the gate implementation — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). Entries change only with explicit user confirmation, recorded in the fixture itself: an entry resolved by user attestation to a new pin carries the attestation date and basis in its `note`, and an entry that a landed engine change deliberately no longer satisfies carries a fixture-level `amendments` record `{user_confirmed: <date>, reason: <basis>}` alongside the gate's KNOWN-LIMITATION list — so every non-passing outcome is machine-visible and auditable. Two entries (`S3_updates_run`, `S13_ricardo_to_cynthia`) SHALL be designated hold-out (not used for any calibration decision).

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file under `openspec/exploration/` at every verification point, so the acceptance evidence is inspectable without re-running. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented with explicit user sign-off via the fixture's amendment/limitation records); or FAIL (blocks the change). A synthetic subset of the gate (the frame/split/attachment rules on recorded fixture arrays) SHALL run in plain `cargo test` without audio or models, and a fixture-lint subset SHALL run in plain `cargo test` asserting every KNOWN-LIMITATION/amendment record carries a user-confirmation date and reason.

#### Scenario: Fixture gate validates the engine before review

- **GIVEN** the meeting audio, local models, and the gate env set
- **WHEN** the gate test runs
- **THEN** every ear-truth entry passes against the derived turns, or each non-passing entry carries a recorded KNOWN-LIMITATION with user sign-off
- **AND** the recorded output file under `openspec/exploration/` reflects the latest run

#### Scenario: Fixture failure blocks the change

- **GIVEN** an engine change that moves a pinned boundary or flips a pinned label
- **WHEN** the gate test runs
- **THEN** it fails, naming the violated entry
- **AND** the entry may only be resolved by passing the engine or by user-confirmed KNOWN-LIMITATION

#### Scenario: Synthetic gate subset runs in CI

- **GIVEN** a plain `cargo test` without meeting audio or models
- **WHEN** the gate's synthetic subset runs
- **THEN** the run/split/attachment rules are asserted against recorded fixture arrays without env gates

#### Scenario: Waivers are auditable

- **GIVEN** a fixture whose KNOWN-LIMITATION list or amendments map contains an entry without a user-confirmation date or reason
- **WHEN** the fixture-lint subset runs in plain `cargo test`
- **THEN** the lint fails, naming the incomplete record
