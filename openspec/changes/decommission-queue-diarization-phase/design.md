## Context

The code sweep (companion commit) deleted the queue's dead diarization transport. Four spec requirements across three capabilities still describe it: the `speaker-diarization` headline requirement, two `post-meeting-pipeline` requirements, and two `recording-lifecycle` requirements.

## Goals / Non-Goals

**Goals:**

- Specs describe the shipped transport: `run_diarization_for_meeting` invoked by import, Enhance/retranscription, and the Speakers commands.
- No silent contradictions left for the next archive to merge.

**Non-Goals:**

- No changes to pipeline substance (chunking, clustering, alignment) — those requirements' bodies are carried over verbatim where transport-neutral.
- No behavior change.

## Decisions

- **REMOVED for post-meeting-pipeline's two queue requirements** — they mandate the deleted machinery exclusively; their live substance (pipeline steps, event payload) is specified by `speaker-diarization`'s headline requirement.
- **MODIFIED (not RENAMED) for the speaker-diarization headline** — archive-time merge is keyed on the requirement name; renaming would complicate the merge. The body's first sentences now state the live transport, superseding the queue-phase framing. The name's "queue phase" wording is historical.
- **MODIFIED for the two recording-lifecycle requirements** — their guarantees (1-second status bar; optional `speaker` field) remain true and live; only the transport clauses change. The `diarization-complete` payload in the delta matches the shipped emitter (`import.rs:729`, `commands.rs:182`): `{meeting_id, speaker_count, segments_labeled}` — the previous `{speakers: [{label, name, color}]}` claim was stale.

## Risks / Trade-offs

- [Headline requirement name still says "queue phase"] → body supersedes; cosmetic rename can ride a future change.

## Migration Plan

Code sweep and this reconciliation land together on the same branch; archive after the identity-stamping change or independently — the two changes touch disjoint requirements.

## Open Questions

- None.
