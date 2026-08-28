## Context

Panel review (three independent adversarial passes) demolished v1 of this design: it targeted `DiarizationProcessor`, which is production-dead (the transcription queue wires `diarization_processor: None`, `transcription_queue.rs:282`). The live store path is `run_diarization_for_meeting` (`commands.rs:518`, hard-codes `None`), and the live matcher is an in-memory registry hydrated once at startup (`setup.rs:37`) via `list_all_embeddings`, keyed by `COALESCE(speakers.name, cluster_label)` and searched at a hard-coded 0.60 (`commands.rs:534`) — contradicting the spec's configurable 0.40 default. Additional live hazards the v1 design missed: `remove_auto_speakers_for_meeting` deletes auto rows on every re-run, and `speaker_embeddings.speaker_id` is `ON DELETE SET NULL` — so any design that lets meeting B's identity anchor to meeting A's auto row gets silently nulled when A re-runs. `revert_speaker_label`'s `cluster_label NOT IN (...)` SQL over- and under-unlinks once rows are stamped. This version is designed around the live path.

## Goals / Non-Goals

**Goals:**

- Live-path stamping: every new embedding row carries a concrete `speakers.id`, written transactionally.
- Cross-meeting identity anchored only to named speakers; pool keyed by speaker id, fresh per run.
- Rename/revert link and unlink embeddings symmetrically by speaker id, fixing the corrupt revert SQL.
- Threshold sourced from settings; legacy NULL rows swept once; NULL stays valid for deliberate unlinking.

**Non-Goals:**

- No change to clustering, temporal smoothing, transcript label writes, or the Speakers-button flow.
- No per-segment rename identity semantics (segment overrides remain label-only string changes, by design — documented here so the divergent semantics is a decision, not an oversight).
- No voice enrollment / name-from-clip feature.
- No backfill of legacy NULL rows (derived data; re-run Speakers per meeting to regenerate).

## Decisions

- **All changes go through `run_diarization_for_meeting`, not `DiarizationProcessor`.** The processor's `match_speakers`/store block stays dead; its doc comment is updated to point at the live path. (v1's fatal flaw.)
- **Named speakers are the only cross-meeting anchors.** Unmatched clusters link to meeting-local auto rows (`speaker-auto-{meeting_id}-*`) that are never match candidates. Alternatives rejected: (a) all stamped rows as candidates — re-running meeting A would null meeting B's identity via `ON DELETE SET NULL`; (b) name-keyed pooling — status quo, "Speaker 0" collides across meetings. Named-only also matches the live spec's "all named speakers" language. Consequence: users assert identity by renaming; the matcher never invents it.
- **Registry keyed by speaker id, refreshed per run.** Extend hydration to key vectors by `speaker_id` and reload the pool at the start of every Speakers run (reuse the hydration function; drop the frozen snapshot semantics). Fixes within-session rename visibility and id stability under renames.
- **Threshold from settings.** Replace the hard-coded `search(&emb, 0.60)` with the configured match threshold (default 0.40, clamp [0.35, 0.70]), resolving the existing spec/code drift.
- **Transactional replacement.** Delete stale + insert stamped rows in one sqlx transaction; propagate errors (fail the run) instead of `log::warn`-and-continue. The pre-run auto-row removal stays outside the tx only if ordering demands it; embeddings themselves are all-or-nothing.
- **Rename/revert link by speaker id, symmetric.** `label_speaker` resolves/creates the target speaker row, then relinks that meeting's embeddings for the cluster — candidates matched by `cluster_label = <original diarization label>` OR `speaker_id = <current speaker id>` (covers unrenamed, manually renamed, and auto-matched badges). `revert_speaker_label` unlinks exactly that candidate set. The `cluster_label NOT IN (...)` SQL is deleted. Tests pin the two corruption cases from the panel (over-unlink on unrelated revert; under-unlink on revert of the renamed cluster itself).
- **Sweep once, then keep NULL legal.** Migration deletes `speaker_id IS NULL` rows. Revert and speaker deletion keep producing NULL rows forever (the live spec mandates unlink-on-revert); NULL simply means "deliberately unlinked", and the matcher ignores NULL permanently. No further sweeps.
- **Prune auto rows on meeting deletion.** Meeting delete cascades embeddings but orphans `speaker-auto-{meeting}-*` rows today; deletion also removes those rows. Named speakers are never auto-pruned.

## Risks / Trade-offs

- [Defaulting the threshold to 0.40 changes auto-match rates vs the de-facto 0.60] → configurable; call it out in release notes; live check on cde5c264 includes a threshold-sensitivity look.
- [Auto rows per meeting grow the speakers table] → pruned on meeting deletion; named speakers are the only long-lived rows.
- [Rename-to-existing-name can merge the wrong person] → revert restores the cluster (previous_label) and unlinks embeddings symmetrically; user-visible merge is intentional per the live labeling spec.
- [Cap-merge blends two people's centroid pre-matching] → clustering-layer behavior, out of scope; the blend links to one identity and revert/re-cluster is the recovery.
- [Migration deletes data] → derived, regenerable per meeting via Speakers; transcripts untouched.

## Migration Plan

1. Ship code (stamping tx, id-keyed refreshed registry, settings threshold, rename/revert linking, auto-row pruning) with RED→GREEN tests against the live command path.
2. Migration sweeps legacy NULL rows at startup (transactional, before any diarization can run).
3. User regenerates identity per meeting by clicking Speakers.
4. Rollback: revert code; swept rows are re-creatable by re-running Speakers.

## Open Questions

- None blocking. (v1's lifecycle question is now decided: prune on meeting deletion.)
