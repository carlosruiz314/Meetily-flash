# Proposal: overlap-separation-prepass

> **UNPARKED 2026-09-25 (later the same day).** The user's ear attested a REAL overlap in
> this meeting: 1055.5-1057.5 (fixture S16) - UserA's 'or IDP rather than search'
> overlapped by a 'What skins?' interjection (probably UserC), whisper interleaving
> the voices out of order with an echo copy (zero walls). This is the change's motivating
> case again, now with attested ground truth and pyannote corroboration (overlap mass
> 0.16-0.26). Scope extension noted: fixing the TEXT (order + echo) needs separated-stream
> re-decode, not just votes - see design addendum. Earlier parking rationale below for the
> record. ** The motivating case (S7c "I don't know") was resolved
> by a different root cause: the user's ear replay proved the words sit at
> 36.1–36.7 — past the engine's 36.08 turn boundary — i.e. whisper DTW smear,
> not a buried voice in a mixture. The user attests there was NO simultaneous
> speech at that span, and pyannote's overlap mass there is only 0.04–0.05.
> The fix shipped as turn-boundary straddle realignment (subturn-voice-
> attribution task 4.8) instead. This change is parked, not rejected: pyannote
> DOES fire high overlap mass elsewhere (0.83 over "Gotcha."'s tail), and if
> the row-voice census ever shows a genuine buried-voice class again, this
> design (port + Conv-TasNet adapter + in-domain quality gate) is the plan.

## Why

The user's ear ruled (2026-09-24, invariant, fixture entry `S7c_idontknow_userA`)
that in meeting cde5c264 the 0.56 s answer "I don't know." (35.51–36.07) is
UserA's voice, but every model channel measures the clip UserB-dominated:
TitaNet whole-atom cosine UB 0.36 vs UA 0.04, no decisive UserA sub-window, an
enriched UserA reference still scores UA 0.02, and pyannote frame masses put
UserA's local voice at 0.01–0.03 while UserB's holds 0.7 solo. The responder
spoke over the asker's tail; the mixture embedding is dominated by the other
voice. No threshold tweak on the current features can recover the buried voice —
the user directed: no hardcoded overrides, algorithms only, maximum feasible
quality; the user selected a source-separation pre-pass as the pursuit.

The class is bigger than one atom: pyannote's overlap mass peaks at 0.6–0.83 in
this meeting alone (over "Gotcha."'s tail), and crosstalk is endemic in
multi-party calls. Today overlap regions get ONE mixture-derived label — the
louder voice absorbs the quieter one.

## What Changes

- Add an overlap-gated source-separation pre-pass: where pyannote's per-frame
  overlap mass indicates two voices, run a speech-separation model on the span,
  embed the separated streams with the existing TitaNet extractor, and let
  those per-stream embeddings vote where the mixture currently votes alone.
- New hexagonal port `VoiceSeparationPort` + adapter over an ONNX separation
  model (Conv-TasNet class: pure 1-D convs, CPU-friendly, 16 kHz pretrained
  variants exist); same ort runtime and model-download pattern as the existing
  pyannote/TitaNet models.
- Mixture embeddings inside high-overlap spans downgrade to abstain evidence;
  separated-stream embeddings carry the vote. Single-voice spans are untouched
  (zero behavior change outside overlap).
- The gate pin `S7c_idontknow_userA` graduates from AMENDED known-limitation
  to a hard pin when the render passes.

## Impact

- Affected: `frontend/src-tauri/src/audio/speaker/` (new `ports`/adapter +
  `run_engine` vote assembly + `commands.rs` model wiring),
  `model_download` (new model artifact), ear-truth fixture (S7c graduation),
  one new gate assertion block.
- Out of scope: any UI surface change (labels flow through the existing
  render), any hardcoded span/label override, streaming/live-call separation
  (batch post-meeting only, same as the rest of the diarization pipeline).
- No new cost on single-voice audio: separation runs only where overlap mass
  fires.
