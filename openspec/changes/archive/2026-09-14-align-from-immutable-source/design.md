## Context

`persist_aligned_groups` DELETES the meeting's transcript rows and replaces them with
the aligned output in the same `transcripts` table. There is no second copy anywhere in
the DB, so `fetch_transcripts_for_alignment` (commands.rs:1301) reads the previous run's
output on every re-run: the Speakers pipeline consumes itself, and each generation
irreversibly loses text structure.

Measured on cde5c264 (align-from-immutable-source proposal): the pre-diarization source
row "Where is Ricardo? I don't know. Let me ping in. I can't." ([32.51, 40.24]) became,
after legacy + driven runs, the fused row "Where is Ricardo I don't know." — the "?"
after "Ricardo" is gone, so the whole-atom aligner (no-split-sentences D1) can never
re-separate the sentences; the fused atom goes WHOLE to the majority badge, misbadging
Cynthia's question. Later runs recorded 237 → 240 → 188 → 173 rows; the count moved
again (182) under a later change's deliberate engine/settings shift, so the exact chain
is historical — the MECHANISM is what matters and is fully verified in code: split rows
persist `token_timestamps = NULL` with fresh UUID ids (speaker.rs:418, 429) and
consolidation deletes absorbed rows, so run N+1's input (row shapes, punctuation,
tokens) is always damaged relative to run N's.

Corroborating evidence from the no-split-sentences design: 237/240 pre-live rows carry
NULL `token_timestamps` — the replaced-row signature. Fresh STT rows always carry the
token JSON. Provenance half-exists today as a side effect; nothing enforces it.

**Adversarial panel finding (round 1, BLOCKING, all four panelists independently):**
the persist path cannot "keep its semantics" under a source/rendering split. It is
keyed by ID EQUALITY: the aligner emits `original_id` = input-row id;
`persist_aligned_splits` looks that id up in `transcripts` and silently returns
`Ok(0)` when absent (speaker.rs:358-362); the absorbed sweep deletes every rendering
row whose id is not an emitted input id (speaker.rs:486-511). After the first run
creates fresh-UUID split/consolidation rows, a second run deletes the entire previous
output as "absorbed" and re-inserts nothing — the exact catastrophic text loss this
change exists to prevent, reintroduced by the persist path. The persist step therefore
requires a REDESIGN (D4), and the main spec's persist doctrine (specs/speaker-
diarization/spec.md:93, 97, 99, 101, 103 — including "split rows carry NULL tokens so
re-diarization never re-expands") is superseded, requiring MODIFIED spec requirements.

## Goals / Non-Goals

- Goal: every Speakers run aligns from the SAME immutable rows the transcription lane
  wrote — full punctuation, token timestamps, original row boundaries.
- Goal: the auto rendering is a pure function of (source rows, engine segments, manual
  overlay): re-running rebuilds it instead of mutating it, so the pipeline can never
  consume its own output again — at the aligner AND at the engine-prior stage (D5).
- Goal: `transcripts` remains the table every existing reader consumes (UI pagination,
  summary chain, search, exports).
- Goal: an escape hatch from already-degraded state: retranscription rewrites the
  source, so a legacy-damaged meeting heals on the next re-transcribe + Speakers run.
- Non-Goal: any engine change (alignment, assignment, borrowing, consolidation,
  clustering are untouched — proposal scope: "no engine change").
- Non-Goal: bit-identical rendering across live re-runs. The pyannote/clustering stage
  is recomputed per run and is not asserted deterministic here (D7); idempotence is
  asserted at the alignment+persist stages, which are pure.
- Non-Goal: recovering text already destroyed in legacy-damaged meetings. Eager backfill
  freezes them as-is; healing is retranscription's job, not a recovery wizard.
- Non-Goal: UI changes (a frozen-degraded source is logged at migration and marked via
  `source_origin`; surfacing it in the UI is future work).

## Decisions

### D1: A second table holds the frozen source — `transcripts` stays the rendering table

`transcript_sources` mirrors the transcription-era column set of `transcripts` — id,
meeting_id, transcript, timestamp, summary, action_items, key_points, speaker,
audio_start_time, audio_end_time, duration, token_timestamps — plus one provenance
column, `source_origin TEXT NOT NULL DEFAULT 'stt'` ('stt' = written by the
transcription lane post-split; 'backfilled' = seeded by the migration). Speaker-lane
columns (`speaker_label`, `speaker_source`, `previous_label`) and `continues_previous`
(a post-insert stamp, commands.rs:1064) are deliberately NOT mirrored — the same
columns today's split INSERT omits. The full mirror
(not a minimal one) exists because the persist path copies template columns
(`timestamp`, `summary`, `action_items`, `key_points`, `speaker`) from the source row
when regenerating rendering rows (D4) — a minimal mirror would not typecheck.
`source_origin` exists because eager backfill makes frozen-degraded sources
indistinguishable from true STT output forever; the column keeps that fact queryable
(no behavior hangs on it in v1). Meeting deletion removes source rows via an explicit
`DELETE FROM transcript_sources WHERE meeting_id = ?` inside `delete_meeting`'s
transaction, mirroring the explicit-delete house pattern (meeting.rs:284-307). FK
cascade is NOT relied on: sqlx-sqlite 0.8 enables `foreign_keys = ON` by default
(verified in the vendored crate source), but every existing child table is deleted
explicitly and `transcript_sources` follows suit.

Rejected alternatives:
- *`transcripts` becomes the source, new rendering table*: conceptually cleanest, but
  every reader (frontend Tauri commands, summary chain, pagination, exports) must switch
  tables — the same guarantee at a far larger blast radius.
- *JSON snapshot per meeting*: lightest migration, but not row-queryable, awkward for
  retranscription's full-meeting replace, and blobs grow unbounded with meeting length.
- *Keeping today's persist and adding a `source_row_id` mapping column*: rejected in
  round 1 — consolidation merges rows derived from MULTIPLE source rows (multi-valued
  mapping), the absorbed-sweep semantics stay intact (the destructive class), and the
  persist step would grow MORE coupling instead of less. Regeneration (D4) deletes the
  whole problem class.

### D2: Eager backfill — the migration seeds `transcript_sources` for every existing meeting

The migration creates the table and copies every meeting's current `transcripts` rows
into it (id preserved), stamping `source_origin = 'backfilled'` on ALL of them — at
migration time provenance is unprovable, so nothing is stamped 'stt'. It logs, per
meeting, the count of backfilled rows whose `token_timestamps` IS NULL (the
replaced-row signature) so frozen-degraded meetings are visible in the log at upgrade
time. Consequences, both intended:

- Never-diarized meetings: their rows ARE true STT output — the copy is exact.
- Legacy-degraded meetings (cde5c264): current rows are diarization output; the copy
  freezes the degradation (no further loss) instead of leaving them on the destructive
  path. Healing path: retranscription deletes and rewrites BOTH tables (D3) with fresh
  full-punctuation Whisper output; the next Speakers run aligns from the healed source.

Rejected: lazy freeze-on-first-run (two read paths live indefinitely) and
true-sources-only backfill via the `token_timestamps IS NOT NULL` predicate (leaves
legacy-degraded meetings degrading forever — the worst outcome for the most damaged
meetings). With eager backfill, the aligner reads the source table UNCONDITIONALLY —
there is no fallback branch anywhere; a dual-write drift (rendering non-empty, source
empty) must instead be OBSERVABLE (D8), not silently absorbed by a fallback.

Migration atomicity: `sqlx::migrate!` wraps each migration in one transaction (no
`-- no-transaction` marker exists in this repo), so a mid-backfill kill rolls back DDL
and data together and re-runs on next launch.

### D3: One shared helper owns source+rendering dual-write; retranscription replaces both

The three fresh-row writers — import.rs:797 (import save), retranscription.rs:658
(retranscription save), transcript.rs:88 (`save_transcript`, used by the recording-stop
path and `api_save_transcript`) — insert into `transcripts` AND `transcript_sources` in
the same transaction via one shared repository helper, so the two tables cannot diverge
by a missed call site. (Round-1 panel verified this writer inventory is complete in
compiled code; every other INSERT hit is `#[cfg(test)]` or the rendering-lane split
insert.) `retranscription.rs:650`'s `DELETE FROM transcripts WHERE meeting_id = ?`
gains the matching source-table delete (a full-meeting source regeneration — this is
the healing path from D2), and `delete_meeting` gains the explicit source delete (D1).

### D4: Persist becomes full-meeting regeneration — the rendering is rebuilt, not mutated

The row-identity hole (Context) is fixed by REMOVING the id-coupling instead of
re-keying it. `persist_aligned_groups` is replaced by a regeneration persist that
writes the aligned output as the meeting's COMPLETE auto rendering in one transaction:

1. Load surviving manual rendering rows: `SELECT id, audio_start_time, audio_end_time
   FROM transcripts WHERE meeting_id = ? AND speaker_source = 'manual'` — empty when
   `rederive_manual` is set (the explicit re-derive path regenerates everything, names
   re-applying via stamped embeddings, unchanged).
2. Suppress any aligned segment whose midpoint falls inside a surviving manual row's
   [start, end] span — the faithful translation of today's guard: a manually-corrected
   row claims its audio span, and fragments covering it are dropped rather than
   duplicated. (In practice the Speakers-button path pre-clears all labels, so the
   manual set is usually empty; the guard is defense-in-depth, same as today.)
3. DELETE all rendering rows of the meeting EXCEPT the surviving manual ids.
4. INSERT every non-suppressed aligned segment as a fresh-UUID row, copying template
   columns from its SOURCE row (join by `original_id` → `transcript_sources`),
   `token_timestamps = NULL`, `speaker_source = 'auto'`, `previous_label = NULL`
   (label history belongs to the surviving manual rows; fresh rows have none).
5. Degenerate-run guard: if the aligned output is empty while the source table is
   non-empty, abort the transaction — never wipe the rendering to nothing on a
   degenerate alignment.

Consequences: the absorbed-row sweep and `delete_absorbed_source_row` are RETIRED (a
regeneration has no "absorbed" class — everything non-manual is replaced by
construction); `persist_aligned_splits`' in-place relabel path is RETIRED (main spec
:99 superseded); and the main spec's "splits carry NULL tokens so re-diarization never
re-expands" idempotency doctrine (:103) is INVERTED — re-diarization now deliberately
re-derives the split shape from immutable source every run, which is only possible
because the input no longer decays. Downstream post-persist steps
(`stamp_continuation_facts`, `consolidate_meeting_turns`) are unchanged: they decorate
and merge freshly-inserted rendering rows. Duplicate-cluster absorption (no-split D4)
already resolves in assembly before persist; its "delete absorbed shells in the persist
transaction" clause becomes moot (there are no shells to delete) — a coordination note
in that change's archive checklist. Same-transaction deletion+insertion preserves the
existing crash semantics (no half-wiped meeting).

### D5: The engine's transcript priors also read the source table

Round-1 panel: the aligner is not the only consumer of rendering rows.
`fetch_transcript_timestamps` (commands.rs:1094-1126, called at :569-571) feeds
`derive_turns` and the legacy fallbacks. Retarget it to `transcript_sources` alongside
`fetch_transcripts_for_alignment` — same immutability argument, same one-line shape.
Without this, a legacy-degraded meeting's run-2 engine priors would be run-1's output
spans: the proposal's headline defect survives at the engine stage.

### D6: Sentence punctuation is a spec-level invariant with a pinned predicate

With the source immutable, every run's aligner input carries full sentence punctuation,
so the whole-atom aligner re-derives the same atoms every run — the erosion mechanism
is structurally gone. The residual risk is the pipeline's own text behavior, so the
spec pins the predicate precisely (round-1 panel: a naive "all punctuation present"
reading fails on legitimate behavior — duplicate clusters write the survivor's text
ONCE, and punctuation-only atoms are dropped by design): **every source sentence
containing at least one alphanumeric character SHALL have its terminator present in at
least one persisted rendering row**, with duplicate-cluster absorbed members,
punctuation-only ranges, and audio claimed by a surviving manually-corrected row
explicitly exempt. A fixture-driven test asserts it.

### D7: Determinism is claimed and tested where the code is pure — alignment + persist

The alignment and persist stages are pure functions of their inputs (round-1 panel
verified: no RNG in `src/audio/speaker`; clustering tiebreaks are index/time-ordered).
The pyannote segmentation/embedding/clustering stage is recomputed per run and is NOT
asserted deterministic by this change. Therefore: the idempotence test (tasks 2.3)
drives the repository with FIXED synthetic diarization segments — twice from the same
source, asserting byte-identical auto rendering (texts, spans, badges; row ids
excepted) — and the spec's idempotence requirement is scoped to the alignment+persist
stages, dropping the "up to engine determinism" escape hatch. The live double-run
check (tasks 3.2) asserts structural stability (row count and text multiset) and
treats badge jitter as a finding to investigate, not a gate.

### D8: Drift and version-skew are observable

Two warning logs, no behavior change: (a) when the aligner fetch finds the meeting's
`transcripts` non-empty but `transcript_sources` empty (dual-write drift, or rows
written by a pre-split binary after a downgrade — the unconditional read then yields
empty and the run would no-op after clearing labels); (b) when regeneration persists
zero segments for a non-empty source (overlaps the D4 abort guard, logged loudly).

## Risks

- *Backfill enshrines degraded text as "source"* for legacy meetings — accepted (D2):
  frozen-degraded beats still-drifting, `source_origin` keeps it queryable, and
  retranscription heals. The fixture `cde5c264_transcripts.pre-live.json` documents
  exactly what such a frozen source looks like.
- *Regeneration widens the blast radius of a bad alignment run*: a degenerate engine
  output replaces the whole rendering instead of patching it — mitigated by the D4
  empty-output abort and the D8 logs.
- *Manual suppression by midpoint containment*: a segment straddling a manual span is
  suppressed or kept whole by its midpoint alone — suppression can drop text outside
  the manual span (retained in source), and a kept straddler can duplicate the manual
  window. Accepted: manual rows usually do not survive a Speakers-button run (labels
  pre-cleared), so the guard is rarely load-bearing; same defense-in-depth status as
  today. The predicate is uniform (midpoint, everywhere in design and spec).
- *Pristine input changes the rejoin units' shape*: `build_logical_units`'
  fragment-rejoin pass was designed against persisted fragment rows; under pristine
  source rows it chains VAD-chopped rows into larger units, and one member failing the
  token clamp degrades the WHOLE unit to proportional spans (blast radius was one
  fragment under fragment input). With the bounded-run guard retired (ear decree),
  atom size is unbounded for unpunctuated backfilled sources. Accepted for this
  change's scope (no engine change): task 2.5 records unit-size and degradation-mode
  observations on the fixture so a follow-up engine change, if needed, is grounded in
  measurement.
- *Dual-write drift* if a future transcription-lane writer bypasses the helper —
  mitigated by D3's single helper plus D8's warning.
- *Storage*: the DB roughly doubles its transcript-row footprint. Transcript text is
  small relative to embeddings/audio; accepted.
- *Main-spec doctrine inversion* (splits CAN re-expand now): the old NULL-token
  rationale existed to freeze split shapes under a decaying input; with immutable
  input, re-derivation is strictly stronger. Coordinated via MODIFIED requirements and
  a reconciliation note against `no-split-sentences`' pending delta, which touches the
  same requirements.
