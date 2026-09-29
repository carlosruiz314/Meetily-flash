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

## Explore-cycle thread resolutions (2026-09-29, second session)

Resolved in dependency order A → B → C: A unblocks task 1.1, B settles the
quality channel C's scoping stance leans on.

### Thread A (must-resolve before 1.1) — duplicate-scan exemption is provenance, not geometry

The ear gate's duplicate scan (`ear_truth_gate.rs`, overlap_pairs) pushes a
render failure for ANY pair of rows at overlapping span walls —
"same-audio double-decode suspects (never dropped)". Two stream rows at
the same overlap span have identical walls BY DESIGN and will trip it.

**Resolution**: exempt on provenance, never on wall-offsetting. Stream
rows inherit the span's source row id as `original_id` (explore finding
2); two rows sharing one `original_id` are attested simultaneous speech —
the both-streams-decisive gate IS the attestation, so no ear ruling is
needed per span. The scan keeps failing any overlapping-wall pair whose
rows trace to DIFFERENT sources (a true double-decode suspect). We do not
"lie about time" (offset walls to dodge the scan); the scan learns what a
legitimate same-wall pair looks like. Lands in task 3.3; adversarial test
9 below pins both sides of the exemption.

### Thread B — confidence channel at the seam, probe before commitment

`whisper_engine::transcribe_audio_with_confidence(Vec<f32>, lang,
offset_ms) -> Result<(String, f32, bool, Option<String>)>` exists and is
the quality channel. Open question: does its confidence discriminate good
stream text from hallucinated/echo text on SEPARATED streams (cleaner
input may score uniformly high, or separation artifacts may score low)?

**Resolution**: run a cheap live probe on the cached separated streams
(env-gated, same pattern as the stop-gate probe) BEFORE finalizing the
seam. If confidence discriminates, the composition-root closure applies a
confidence floor (plus the bool hallucination flag) and returns `None`
below it — the closure stays `&dyn Fn(&[f32]) -> Option<String>`, the pure
function signature is untouched, and "None from the seam" is just the
existing no-synthesis degrade. If it does not discriminate, the
hallucination guards alone carry the weight and the closure stays plain.
Probe folds into task 2.1.

### Thread C (scoping stance) — uniform rule + gate census

Synthesize ALL spans meeting the both-streams-decisive gate
meeting-wide (uniform), vs only ear-attested spans (S16 only).

**Resolution**: uniform + gate census. Selecting spans by hand would be a
hardcoded override in disguise; the uniform rule is driven entirely by the
evidence channels (votes + margin + text guards). The ear gate logs a
census of every synthesized span (walls, both voices, per-stream text from
the local fixture) so Phase 4 ear sampling draws from the complete
inventory — the user hears a representative sample, not a curated one.
S16 stays special only in the fixture's expected needles (task 3.3), not
in the rule.

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
9. Duplicate-scan exemption is provenance-scoped (thread A): a same-wall
   stream pair sharing one `original_id` passes the scan; an
   overlapping-wall pair tracing to different sources still fails it.

## §3 smoke-spec decision

The synthesis surfaces through the existing transcript-render flow; the
Speakers diarization run itself is not Playwright-drivable (real models +
recording, same rationale recorded by subturn-voice-attribution and
overlap-separation-prepass 3.3). The render change is pinned by the offline
ear gate (assertion class above) + the persistence round-trip test; if apply
touches a Tauri command surface, the smoke spec is added in that task per
the standing rule.
