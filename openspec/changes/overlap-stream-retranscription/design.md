# Design: overlap-stream-retranscription

## Hexagonal boundaries

- **Port (reuse)**: the separation pre-pass's `VoiceSeparationPort` —
  unchanged.
- **Transcription seam (new, first caller)**: the diarization pipeline has
  never needed Whisper; this change does. Rather than import
  `whisper_engine` deep in the aligner, `commands.rs` (composition root)
  holds the loaded `TranscribePort` handle it already builds for the main
  transcription path and passes a `&dyn Fn(&[f32]) -> Option<String>` to the
  synthesis pass. This is the §8 `hexagonal-port-traits` forcing function
  arriving from a real §4 need; the closure is the seam until the deferred
  port-traits change lands.
- **Use case (new, pure)**: `run_assembly::synthesize_overlap_rows` — pure
  function, no I/O: inputs are the existing rows, the separation votes,
  covered atoms, and per-stream texts; output is the synthesized row list.
  Fully unit-testable with fakes.
- **Adapter (existing)**: `whisper_engine` for stream transcription
  (16 kHz mono in); the hallucination-repair path (language pin, quarantine,
  degenerate-repeat guards) runs on stream text exactly as on live
  transcription output.

## Key decisions

1. **Render-level only.** `transcript_sources` is immutable (spec hard
   invariant). Stream rows exist in the regenerated rendering only; they
   carry their source row ids so persistence and undo flows can trace them.
2. **Both-streams-decisive gate.** Rows are synthesized only when the
   existing margin-gated votes identified BOTH streams (one vote each, ≥
   margin) AND each stream's text survives the hallucination guards
   (non-empty, not quarantined). Anything less degrades to the mixture rows.
3. **Span walls, not token walls (v1).** Stream rows span the overlap span
   itself. Token timestamps on separated audio are DTW-hostile (the S7c
   lesson); word-level splitting inside streams is deliberately out of scope.
   The no-split-sentences invariant holds by construction: one row per
   voice, one badge per row.
4. **Text replacement is honest.** The mixture row's text is REPLACED in the
   render because it is the wrong artifact (interleaved + echo). The echo
   dedup and degenerate-repeat guards still apply per stream (whisper can
   echo a separated stream too).
5. **Cost is bounded**: two whisper inferences per attested overlap span
   (measured: 38 spans meeting-wide, each ≤ a few seconds) on an already-
   loaded model.

## Interactions with existing mechanisms

- **Separation pre-pass**: consumes its port, its `overlap_spans`, its
  covered-atom filter, and its ref-anchored votes verbatim. No new trigger.
- **Wall realignment / voice votes**: synthesized rows enter the render
  AFTER alignment as replacements for the span's rows; they cannot straddle
  boundaries (single span, single voice each), so no new interaction with
  `realign_straddling_atoms`.
- **Ear truth gate**: gains one new assertion class — the S16 window must
  contain two distinct-badge rows whose per-voice text reads in order; the
  gate reads expected needles from the local fixture (real text never
  enters code). It also needs the provenance-scoped duplicate-scan
  exemption (thread A below) or its own overlap scan will fail the render
  this change creates.

## Explore-cycle findings (2026-09-29, pre-apply audit)

1. **Whisper seam resolved to a concrete handle**: `whisper_engine::commands::
   WHISPER_ENGINE` (static `Mutex<Option<Arc<WhisperEngine>>>`) with
   `transcribe_audio(Vec<f32>, lang) -> Result<String>`. The closure is built
   from that static at the composition root; an UNINITIALIZED engine (real
   case: probe/test contexts) is the degrade path, not a hypothetical.
2. **Persistence verified, one trap found**: `persist_regenerated_rendering`
   writes `seg.text` (the RENDER text) into fresh-UUID rendering rows and
   takes template metadata from `transcript_sources` keyed by
   `original_id` — so stream text persists as-is. TRAP: a rendering row
   whose `original_id` is absent from `transcript_sources` is silently
   SKIPPED at persist. Stream rows MUST inherit the span's source row id as
   `original_id` (tested in 1.1).
3. **Replacement is already the persist model**: rendering rows are deleted
   and reinserted (fresh UUIDs) on every persist — stream rows are ordinary
   rendering rows; no special replacement mechanics exist or are needed.
4. **Gate snapshot ordering**: the gate pins the replay to a row snapshot
   AND cross-checks the live DB hash — after the render changes, the
   snapshot must be re-pinned AFTER the live persist (task 3.5), not before.
5. **Async bridge at the seam is a non-problem**: `decode_with_params` is
   async only for the tokio `RwLock` on the model context
   (whisper_engine.rs:841); the whisper.cpp call itself is synchronous. The
   seam closure uses a sync decode method (`blocking_read` over the same
   lock) — legal because the speaker pipeline already runs inside
   `spawn_blocking` (commands.rs:645+), where blocking primitives are
   permitted.

## Explore-cycle thread resolutions (2026-09-29, second session)

Resolved in dependency order A → B → C: A unblocks task 1.1, B settles the
quality channel C's scoping stance leans on.

### Thread A (must-resolve before 1.1) — duplicate-scan exemption is provenance, not geometry

The ear gate's duplicate scan (`ear_truth_gate.rs`, overlap_pairs) fails ANY
pair of GROUPS at overlapping walls — and groups are merged turns, not raw
rows, so a stream row can also end up glued to an adjacent same-speaker
neighbor (gap ≤ 3 s). Two stream rows at the same overlap span trip it
either way.

**Resolution**: exempt on provenance, never on wall-offsetting. Verified
mechanics: rows flow through the pipeline as `AlignedSegment`, which
ALREADY carries `original_id` end-to-end (`merge_same_label_fragments`
compares it; there is a `never_crosses_source_rows` test). The exemption
therefore needs NO change to `turns.rs` and no new plumbing in the
pipeline — the gate's `RowRef` projection is what drops the id. The gate
keeps a parallel `original_id` list aligned with its `RowRef` slice;
`TurnGroup.row_indexes` already indexes that slice, so a group's
provenance is the id set of its absorbed rows. Exemption rule at GROUP
level: overlapping-wall groups whose id sets INTERSECT are attested
simultaneous (the stream pair shares the span's source row id — the
both-streams-decisive gate IS the attestation); disjoint id sets at
overlapping walls remain true double-decode suspects and still fail. We
do not "lie about time" (offset walls to dodge the scan). Lands in task
3.3; adversarial test 9 pins both sides.

**New hazard found while pulling (test 10)**: the replay chain runs
`resolve_duplicate_clusters` (production D4) between merge and persist.
The two stream rows share walls and MAY share token-similar text (an echo
or a genuine repeat) — the resolver could classify them as a
re-transcription cluster and DROP a voice. The synthesis pass MUST insert
AFTER that resolver (stream rows are replacements, not a re-decode to
dedup), pinned by a test: the stream pair survives the resolver.

### Thread B — confidence channel is DEAD on arrival; guards carry the weight alone

The planned live probe is cancelled — no probe can rescue this value.
`decode_with_params` (whisper_engine.rs ~:872) computes the returned
"confidence" as `(segment_text.len() / 100.0).min(0.9) + 0.1`: a
TEXT-LENGTH proxy, not whisper.cpp's avg_logprob. Hallucinated fluent
fictions are long, so the proxy is anti-correlated with the exact failure
mode this change must reject. The tuple's `bool` is `is_partial`
(duration < 15 s), not a hallucination flag. The engine's own params
comment already records the real design: gates inside whisper.cpp are
advisory (last decode is still emitted) and "the lane audits text after
decode (audio::hallucination)".

**Resolution**: the composition-root closure stays plain
`&dyn Fn(&[f32]) -> Option<String>` with NO confidence floor; quality is
enforced by the hallucination lane (degenerate-repeat guard, quarantine,
echo dedup) exactly as the both-streams gate already assumed. The pure
function signature is untouched.

### Thread C (USER-DECIDED 2026-09-29) — run everywhere, calibrate by ear, flags never drop

The user ruled: run the talk-over moments machine-wide, then hand them a
sample of clips (up to ~20) to confirm by ear which are genuine talk-overs
and whether the lines read right. Motivation on record: the detector has
dials (trigger mass, vote margins) and may fire too often — over-firing
would produce MORE hallucinated streams, not fixes. Protocol:

1. **Uniform run**: every span meeting the both-streams gate is
   synthesized. No hand-picked list; ear rulings tune the general dials,
   never a per-moment skip list.
2. **Census + word-loss flags**: the gate logs every synthesized span —
   walls, both voices, per-stream text, the trigger mass and vote margins
   that fired it, and a word-loss flag (synthesized rows' word count vs
   the mixture row's). FLAG-ONLY: a flagged span still renders its
   synthesized rows — no automatic dropping or reverting at this
   calibration stage (user explicit).
3. **Clip set (Phase 4)**: up to ~20 moments for the user's ear —
   word-loss-flagged moments first, remainder evenly spaced through the
   meeting (mechanical, not curated). Before sending, the agent reads the
   full rerun transcript against the original and reports whether it makes
   more or less sense (the agent's read is a signal, the user's ear is the
   judge).
4. **Graduation bar unchanged**: S16's fixture needles stay the regression
   pin; the 33.2–38.5 window proves no synthesis outside real overlaps.

Cost bound: worst case 2 inferences × 38 spans ≈ 76 short inferences on an
already-loaded model.

## Security / trust boundaries

Stream text is untrusted model output: it passes the same schema validation
and hallucination guards as live transcription before touching the render.
No LLM in this path. The separator's output never reaches persistence
(audio only becomes text via Whisper; embeddings never leave the process).

## Adversarial tests (RED before GREEN)

1. Silent stream (RMS-floor passed but no speech) → whisper returns empty →
   NO synthesis; mixture rows survive.
2. Hallucination stream (whisper echoes a nonexistent sentence) → caught by
   the existing degenerate-repeat guard → NO synthesis for that stream.
3. Margin miss on either stream → NO synthesis (mixture rows survive).
4. Span clamped at recording edges → no panic, no synthesis.
5. Immutability: a full run leaves `transcript_sources` byte-identical while
   the regenerated rendering contains the stream rows.
6. Byte-identical degrade: separation model missing / whisper unavailable →
   render identical to today's (existing channel).
7. Persistence round-trip: stream rows persist with their per-stream badge
   and survive a refetch (DB-layer verification, terminal-only rule).
8. Gate: S16 window renders two distinct-badge in-order rows; the 33.2–38.5
   regression pins stay green (no synthesis outside overlap spans).
9. Duplicate-scan exemption is provenance-scoped (thread A): two
   overlapping-wall groups whose absorbed-row id sets intersect (stream
   pair sharing the span's source id) pass the scan; a disjoint-id
   overlapping-wall pair still fails it.
10. Stream pair survives `resolve_duplicate_clusters`: synthesis inserts
    after the D4 resolver, and a same-span stream pair with token-similar
    text is never classified as a re-transcription cluster (no voice
    vanishes).
11. Word-loss flag is diagnostic only: a span whose synthesized rows lose
    words vs the mixture row is flagged in the census and still renders
    its synthesized rows — the flag never drops or reverts anything.

## §3 smoke-spec decision

The synthesis surfaces through the existing transcript-render flow; the
Speakers diarization run itself is not Playwright-drivable (real models +
recording, same rationale recorded by subturn-voice-attribution and
overlap-separation-prepass 3.3). The render change is pinned by the offline
ear gate (assertion class above) + the persistence round-trip test; if apply
touches a Tauri command surface, the smoke spec is added in that task per
the standing rule.
