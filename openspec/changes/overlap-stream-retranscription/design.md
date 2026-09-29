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
  enters code).

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

## §3 smoke-spec decision

The synthesis surfaces through the existing transcript-render flow; the
Speakers diarization run itself is not Playwright-drivable (real models +
recording, same rationale recorded by subturn-voice-attribution and
overlap-separation-prepass 3.3). The render change is pinned by the offline
ear gate (assertion class above) + the persistence round-trip test; if apply
touches a Tauri command surface, the smoke spec is added in that task per
the standing rule.
