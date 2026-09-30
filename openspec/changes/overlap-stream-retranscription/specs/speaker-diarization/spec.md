## ADDED Requirements

### Requirement: Overlap spans render per-voice rows from separated streams

When the separation pre-pass identifies an overlap span and the margin-gated
voice votes are decisive for BOTH separated streams (one vote each, at or
above margin), the pipeline SHALL transcribe each separated stream (span
carved with context padding, RMS-normalized) through the loaded Whisper
engine, and the span's mixture rows in the regenerated rendering SHALL be
replaced by one row per decisive stream — span walls, the stream's voice
badge, the stream's text. The two streams' decisive votes SHALL resolve to
DIFFERENT speaker clusters; a same-badge outcome (separation collapse) SHALL
degrade to the mixture render. Vote evidence SHALL carry stream identity (a
per-span record of stream → cluster, margin, text) so badges are not derived
from vector order. The two stream rows SHALL render in a deterministic
order pinned by test.

Each stream row SHALL inherit the span's source row id as `original_id` so
persistence and undo flows can trace it (a rendering row whose
`original_id` is absent from `transcript_sources` is silently dropped at
persist). When a span's covered atoms intersect multiple source rows, each
stream row SHALL inherit the id of the source row with maximum covered-atom
overlap (tie: earliest start); the selection SHALL be pinned by test.
Replacement scope is likewise pinned: a mixture row extending beyond the
span walls either keeps its out-of-span remainder or is replaced whole —
the rule SHALL be stated in the change's design and tested so no out-of-span
words are silently lost.

The stream decode SHALL use the meeting's resolved language — the concrete
code when the user preference names one; automatic preference states SHALL
degrade (no synthesis), because the automatic resolution is translation and
stream rows translated to English would be the defect. Never
`auto-translate`, never per-stream auto-detection. Streams SHALL decode under a deterministic
profile (pinned language, greedy search, temperature 0) rather than the
live chunked profile, and the profile SHALL NOT consume token timestamps
(stream rows use span walls). A stream SHALL be eligible for synthesis only
if its PRE-normalization span RMS is at least a stated fraction of the
mixture clip's RMS (recorded in the census); the post-normalization RMS
floor alone is not eligibility evidence, because normalization amplifies
near-silence into the hallucination regime.

The synthesis pass SHALL run after duplicate-cluster resolution — a
same-span stream pair SHALL never be classified as a re-transcription
cluster and dropped. Stream text is untrusted model output: it SHALL pass
the hallucination guards (non-empty after trimming, not quarantined,
degenerate-repeat and echo checks) exactly as live transcription output
does.

If any condition fails — vote margin miss on either stream, both streams
resolving to the same badge, empty or quarantined stream text, pre-
normalization RMS below the eligibility floor, separation model missing,
transcription engine unavailable, or the span containing a surviving
manually-labeled row (the manual row wins; synthesis degrades for that
span) — the rendering SHALL be byte-identical to today's mixture render.
`transcript_sources` SHALL remain byte-identical across the whole flow.

Stream rows SHALL comply with the persisted hard invariants: each carries
the continuation fact (`continues_previous = true` when the stream's text
begins mid-sentence), and a stream row is a synthesis atom — the same-
speaker merge and any later consolidation SHALL NOT re-merge it into an
adjacent same-speaker turn. A word-loss diagnostic SHALL be computed per
synthesized span — duration-normalized rates (words per second over the
span, flagged in both directions: too low suggests lost words, too high
suggests hallucination) with the raw-count comparison against the SUM of
all replaced mixture rows as a secondary number — and a cross-stream
duplication diagnostic SHALL flag stream pairs whose texts share a
contiguous ≥3-token chunk covering ≥80% of the shorter (suspected separator
leak). Both diagnostics are flag-only: they SHALL NOT drop or revert any
row.

#### Scenario: Decisive overlap span renders two per-voice rows

- **GIVEN** an overlap span where both separated streams have
  margin-passing voice votes for DIFFERENT clusters and guard-clean text
- **WHEN** the rendering regenerates
- **THEN** the span's mixture rows are replaced by two rows — one per
  stream — with span walls, each stream's voice badge, and a deterministic
  order
- **AND** each stream row's `original_id` is the pinned source-row
  selection for that span

#### Scenario: Undecidable span degrades to the mixture render

- **GIVEN** an overlap span where a vote margin misses, both streams vote
  the same cluster, a stream's text is empty (including whitespace- or
  punctuation-only) or quarantined, a stream's pre-normalization RMS is
  below the eligibility floor, the separation model is missing, the
  transcription engine is unavailable, the span is clamped at a recording
  edge, or a manually-labeled row survives inside the span
- **WHEN** the rendering regenerates
- **THEN** the span renders exactly today's mixture rows, byte-identical

#### Scenario: Immutability and persist round-trip

- **GIVEN** a full diarization run that synthesized at least one overlap
  span
- **WHEN** the render persists and is refetched
- **THEN** `transcript_sources` is byte-identical to before the run
- **AND** the persisted stream rows keep their per-voice badges and text

#### Scenario: Word-loss and cross-stream flags are diagnostic only

- **GIVEN** a synthesized span whose stream rows lose words vs the replaced
  mixture rows, or whose two stream texts are near-identical
- **WHEN** the render and the census are produced
- **THEN** the span is flagged in the census for the respective diagnostic
- **AND** the span still renders its synthesized rows — nothing is dropped
  or reverted by a flag

#### Scenario: Stream pair survives duplicate-cluster resolution

- **GIVEN** a same-span stream pair whose texts are token-similar (an echo
  or a genuine repeat)
- **WHEN** the duplicate-cluster resolver runs
- **THEN** both stream rows survive — no voice vanishes

#### Scenario: Every gate-decisive span is synthesized — no skip list

- **GIVEN** the set of overlap spans meeting the both-streams-decisive gate
  across a meeting
- **WHEN** the rendering regenerates
- **THEN** every span in that set is synthesized — no span is excluded by
  any curated list; ear rulings tune the general thresholds, never
  individual spans

#### Scenario: Manual row wins over synthesis

- **GIVEN** a span where the user manually labeled a row in a previous run
- **WHEN** the next diarization run regenerates the render
- **THEN** the span does not synthesize (the manual row survives as today)
- **AND** the other voice's fresh row is not suppressed by midpoint
  suppression

## MODIFIED Requirements

### Requirement: Persisted speaker turns are sentence-readable

The diarization persist path SHALL write speaker turns, not raw aligned fragments: adjacent rows of the SAME speaker with a time gap of at most 3 seconds SHALL merge into one turn (text joined in time order); rows that carry no alphanumeric content SHALL be dropped; word-joined text SHALL be detokenized (no space before sentence punctuation, contractions reattached). Rows of DIFFERENT speakers SHALL never merge, even across a mid-sentence interjection. Synthesized overlap-span stream rows are exceptions to the merge rule: they are synthesis atoms — the merge SHALL NOT fold a stream row into an adjacent same-speaker turn, and consolidation SHALL treat stream rows as atoms so re-running it never unwinds the per-voice render.

#### Scenario: Same-speaker fragments merge into a readable turn

- **WHEN** alignment produces "Speaker 0: `. Okay . I have some updates . Cool . On the`" followed 0.4 s later by "Speaker 0: `to , let 's , wait`"
- **THEN** one persisted row for Speaker 0 reads "Okay. I have some updates. Cool. On the to, let's, wait" spanning both time ranges
- **AND** the original fragments no longer exist as separate rows

#### Scenario: Backchannel fragment merges; junk rows disappear

- **WHEN** a 0.5 s row reading "Yeah ," sits between two Speaker 1 rows, and a row reading "," sits anywhere
- **THEN** the "Yeah ," row merges into its neighboring same-speaker turn
- **AND** the punctuation-only row is dropped entirely

#### Scenario: Speaker flip never merges, mid-sentence

- **WHEN** Speaker 0's fragment "On the" is followed by Speaker 1's "roadmap , hopefully"
- **THEN** both persist as separate rows (the interjection is real)
- **AND** Speaker 0's text is detokenized ("On the", no trailing-space artifacts)

#### Scenario: Silence longer than 3 seconds starts a new turn

- **WHEN** two same-speaker rows are separated by more than 3 seconds of gap
- **THEN** they persist as separate turns

#### Scenario: Stream row is not re-merged into a same-speaker neighbor

- **GIVEN** a synthesized stream row adjacent to a same-speaker row from
  outside the span (gap ≤ 3 s)
- **WHEN** assembly or a later consolidation pass runs
- **THEN** the stream row persists as its own row with its own badge and
  text

### Requirement: Sentences are not split across speaker badges

The diarization assembly SHALL treat the transcript sentence as the atom of speaker assignment. Persisted fragment rows that are adjacent in the persisted sequence and whose predecessor lacks sentence-terminal punctuation SHALL first be REJOINED into one logical text unit (a text-level repair spanning badges and row gaps — not a badge merge; rejoined text preserves word order and content). Each sentence (segmented from the logical units, with a time span from valid token timestamps or a proportional share of the unit's span) SHALL then be assigned WHOLE to one speaker badge: the engine turn owning the majority of the sentence's span. A sentence SHALL NOT be divided across two speaker rows. The engine's turn boundaries themselves are unchanged, and this SHALL NOT be implemented as a merge of two speakers' rows — minority-span words move with their sentence (reattribution), and a turn whose span holds no assigned sentence emits no row.

**No bounded-run guard (user ear decree, 2026-09-09 — supersedes the original 8 s cut design)**: the voice does NOT change mid-sentence in the user's meetings. Every engine boundary inside a sentence is an engine error, absorbed by whole-atom majority assignment; sentence atoms are never cut and never flagged as continuation tails. The bounded-run cut, its `cross_badge_tail` flag, and the fracture waiver class are RETIRED. The fix for a wrong badge is engine boundary accuracy (change `engine-boundary-and-identity-accuracy`), never cutting the sentence.

The render gate SHALL count cross-badge fractures with the predicate "row begins mid-sentence AND the previous persisted row has a different badge AND that previous row does not end with terminal punctuation" and SHALL fail on any fracture with NO waiver path — there is no amendment-record waiver for fractures. Lowercase sentence onsets that are the ASR text itself (no cross-badge fracture) are NOT violations. One provenance-scoped exemption exists: two synthesized overlap-span stream rows sharing one source row id at identical walls are ATTESTED SIMULTANEOUS speech — their cross-badge adjacency is the attested phenomenon, not a sentence cut — and SHALL NOT count as a fracture; the exemption never applies to rows with disjoint source ids. The ear gate SHALL additionally count duplicate clusters and overlapping-span rows (same-audio double-decode suspects), and report churn counters (persisted rows per minute; rows of ≤2 words).

Adjacent persisted rows whose normalized token sequences share a contiguous subsequence of at least 3 tokens covering at least 80% of the shorter row — with disjoint time spans, an inter-row gap of at most 2 seconds, and different badges (or one side "Unknown Speaker") — form a duplicate cluster (a chunk-overlap re-transcription). Assembly SHALL resolve a duplicate cluster by keeping ONE copy — survivor badge: labeled over "Unknown Speaker"; among multiple labeled rows, the badge owning the majority of the cluster's union span; final tiebreak earliest start — writing its text ONCE, extending the survivor's span to the union of the cluster's spans, and deleting the absorbed row shells inside the persist transaction; no source row may survive the transaction as an unlabeled orphan. The survivor's time coverage SHALL never shrink (audio-time evidence is preserved).

#### Scenario: Turn flip inside a sentence keeps the sentence whole

- **WHEN** the sentence "Where is UserC?" spans a speaker flip between Speaker 1's turn and Speaker 0's turn
- **THEN** the full sentence persists under exactly one badge (the majority-span badge)
- **AND** no row under the other badge contains any part of that sentence
- **AND** the engine's turn set still contains the flip (engine boundaries are not edited)

#### Scenario: Mid-sentence tail under a different badge is repaired by rejoin

- **GIVEN** persisted fragment rows "I" (Speaker 1, 39.27–39.93) followed by "don't know. Let me ping in..." (Speaker 0), where "I" lacks sentence-terminal punctuation
- **WHEN** assembly rejoins the adjacent fragments into one logical unit, segments it into sentences, and assigns each sentence to its majority-span badge
- **THEN** "I don't know." persists whole under Speaker 0 (the majority badge), not split as "I" under Speaker 1 with the tail under Speaker 0
- **AND** any residual fracture that cannot be repaired trips the gate with the offending rows

#### Scenario: ASR-lowercase onset without fracture is not a violation

- **GIVEN** a row that begins with a lowercase word because the ASR text itself is unpunctuated lowercase (e.g. "yeah i think it's two sprints..."), whose previous row ends with terminal punctuation or carries the same badge
- **WHEN** the render gate's fracture predicate evaluates it
- **THEN** the row is not counted as a violation

#### Scenario: Short interjection does not slice the host sentence

- **WHEN** a short interjection sentence ("Yeah,") from Speaker 0 lands between two sentences of Speaker 1
- **THEN** the interjection persists as its own row under Speaker 0
- **AND** both surrounding Speaker 1 sentences persist whole under Speaker 1

#### Scenario: Duplicate re-transcription cluster is merged, not double-persisted

- **GIVEN** two rows 1.1 s apart with disjoint spans, different badges, whose texts share the contiguous 6-token sequence "motors we're doing the feature flag update" covering ≥80% of the shorter row
- **WHEN** assembly resolves the duplicate cluster
- **THEN** one row survives carrying the shared text once, with its span extended to the union of the cluster's spans
- **AND** the absorbed row's shell is deleted in the same transaction and no unlabeled source row remains
- **AND** a genuine short repeat ("I can't. I can't." inside one row, or a full-sentence interjection) is not a duplicate cluster (its atoms are not a ≥3-token contiguous cross-badge match with disjoint spans)

#### Scenario: Render-level residuals other than fractures are waivable only by amendment record

- **WHEN** a render-level gate assertion (duplicate, unknown-within-cap, zero-duration, unmerged) has an irreducible residual on the pinned fixture
- **THEN** the gate fails unless the fixture carries a user-signed amendment record for that offender (date + reason), the same mechanism as fixture-entry amendments
- **AND** a cross-badge fracture has no such path — since the 2026-09-09 ear decree every fracture fails hard

#### Scenario: Attested simultaneous stream pair is not a fracture

- **GIVEN** two synthesized stream rows at identical walls sharing the
  span's source row id, the first not ending in terminal punctuation
- **WHEN** the render gate's fracture predicate evaluates the pair
- **THEN** the pair is not counted as a fracture (attested simultaneous
  speech)
- **AND** a mid-sentence-initial row following a different-badge row with
  disjoint source ids still fractures

### Requirement: Ear-truth fixture gate validates attribution

The repository SHALL contain a pinned ear-truth fixture (`frontend/src-tauri/tests/fixtures/ear_truth_cde5c264.json`) holding attribution facts as data, each entry `{id, start_s, end_s, kind, params}` with kinds: `single_voice` (all turns overlapping the span carry one label — silence-delimited same-speaker boundaries inside the span are not violations, since the ear attests voices, not turn units), `voice_change_at` (exactly one label change inside the span, one within the pinned tolerance; the pinned text tail belongs to the earlier turn), `multi_voice` (at least one label change inside the span — for attested trading with an unattested count), `distinct_speaker` (the span's turn label differs from the surrounding turns'). The 13 recorded entries (user-ear answers of 2026-09-04, verbatim in the local fixture's answers record, resolved to absolute times via recovered clip offsets): single-voice spans ≈5.9–12.8 (the user's sentence), 15.5–20.8, 24.5–29.5 (UserB), 32.0–38.0, 2803–2820 (UserC); voice changes at ≈13.0 (user→UserB), ≈29.5 (UserB→user), ≈31.5 (user→UserB one-word backchannel), ≈38.0/≈39.0 ("okay" interjection), ≈2776.4 (two voices trading), ≈2803.0 (UserB→UserC), ≈2821.0 (UserC→UserB), and the 02:12–02:50s anchor `voice_change_at` ≈161s ±0.75 with the pinned mid-sentence tail on the earlier side (verbatim tails live only in the local fixture). Entries change only with explicit user confirmation, and two entries (`S3_updates_run`, `S13_userC_to_userB`) SHALL be designated hold-out (not used for any calibration decision). [Restatement note: two needle descriptions that were verbatim in the live text are tokenized here under the PII bright line; the live-spec scrub is tracked as a separate change.]

A gate test SHALL run the turn-derivation engine on the real meeting audio and assert every entry, failing with the entry name on mismatch. Because it requires the meeting audio and local models, the gate SHALL be env-gated like the existing live diagnostics, AND a named runner script SHALL record the gate output to a file — TOKEN-ONLY content — at every verification point, so the acceptance evidence is inspectable without re-running; recorded artifacts and their directory SHALL be gitignored and added to the pre-push guard's protected pathspecs. Per-entry outcomes SHALL be exactly: PASS; KNOWN-LIMITATION (documented in this change with explicit user sign-off); or FAIL (blocks the change). A synthetic subset of the gate (the frame/split/attachment rules on recorded fixture arrays) SHALL run in plain `cargo test` without audio or models.

The gate's duplicate scan SHALL be provenance-scoped at group level: overlapping-wall turn groups whose absorbed-row id sets intersect (the synthesized stream pair shares the span's source row id — the both-streams-decisive gate is the attestation of simultaneity) SHALL pass the scan as attested simultaneous speech; overlapping-wall groups with disjoint id sets remain double-decode suspects and SHALL still fail the scan. Wall-offsetting (shifting timestamps to dodge the scan) SHALL NOT be used. The gate SHALL log a census over every trigger-fired overlap span — synthesized or not — recording: walls, the span's trigger mass, per-stream best and second-best similarities and margins (including REJECTED spans, so threshold retuning has the near-miss distribution), per-stream pre-normalization span RMS against the clip's RMS, the synthesis outcome with both voice badges, per-stream text ONLY as token-only data (word/character counts and a sha256 of each stream's text), the word-loss diagnostic, and the cross-stream duplication diagnostic. Recorded census artifacts SHALL be token-only; verbatim per-stream text SHALL be emitted only to the terminal under an env gate or stored in the private evidence home — never in a repo-committed file. The census is diagnostic and never drops rows. The gate's S16 assertion requires a real loaded Whisper engine in the gate process (the uninitialized-engine degrade would otherwise suppress every synthesized row); the census records the model name used.

#### Scenario: Fixture gate validates the engine before review

- **GIVEN** the meeting audio, local models, and the gate env set
- **WHEN** the gate test runs
- **THEN** every ear-truth entry passes against the derived turns, or each non-passing entry carries a recorded KNOWN-LIMITATION with user sign-off
- **AND** the recorded output file reflects the latest run and contains
  token-only content

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

#### Scenario: Census carries the retuning evidence, token-only

- **GIVEN** a gate run over a meeting with trigger-fired spans, some
  synthesized and some rejected by the vote margin
- **WHEN** the census is recorded
- **THEN** every trigger-fired span appears with its trigger mass, both
  streams' best/second similarities and margins, and pre-normalization RMS
  ratios — including rejected spans
- **AND** no verbatim meeting text appears in any recorded artifact (text
  is counts + sha256 only)
