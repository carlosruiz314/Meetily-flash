# Design: overlap-separation-prepass

## Hexagonal boundaries

- **Port** (new): `frontend/src-tauri/src/audio/speaker/ports/voice_separation.rs`
  — `trait VoiceSeparationPort { fn separate(&self, samples: &[f32], span_secs: (f64, f64)) -> anyhow::Result<Vec<SeparatedStream>>; }`
  where `SeparatedStream { samples: Vec<f32> }` (one entry per recovered voice,
  unordered). Depends on domain types only.
- **Adapter** (new): `frontend/src-tauri/src/audio/speaker/separation/conv_tasnet.rs`
  — ort session over the ONNX model, 16 kHz in/out, windowed inference over the
  span with the model's expected segment size. Implements the port. Model file
  ships via the existing `model_download` pattern (hash-pinned artifact in the
  models dir next to pyannote/TitaNet).
- **Use case wiring**: `run_engine` computes overlap spans from the frame
  masses it already holds (`FrameMasses.overlap`); for each span above
  `OVERLAP_TRIGGER_MASS` sustained ≥ `OVERLAP_MIN_SPAN_SECS`, call the port,
  embed each stream per wall-atom geometry (reuse `token_wall_atoms`), and emit
  additional margin-gated votes into the SAME `voice_votes` channel the render
  already consumes. No render-layer change at all.
- **Composition root**: `commands.rs` loads the model next to the other two;
  model-missing/corrupt degrades to today's behavior (no separation votes),
  logged — never a run failure.

## Key decisions

1. **Conv-TasNet class over SepFormer**: pure 1-D convs export to ONNX without
   dynamic ops and run CPU-fast; SepFormer's dual-path transformer is too slow
   on CPU for 83-minute meetings even gated. Candidate: Asteroid 16 kHz
   2-speaker checkpoints (e.g. ConvTasNet_Libri2Mix family) exported via
   torch.onnx. Selection task includes an IN-DOMAIN quality gate before
   commitment (see adversarial tests): WSJ0/LibriMix models are trained on
   clean read speech — meeting audio (codec, room noise, 3+ voices) is
   out-of-domain, and a bad separator is worse than none.
2. **Overlap-gated, not always-on**: `FrameMasses.overlap` (powerset classes
   4–6, already decoded per frame) is the trigger and the budget. Measured on
   cde5c264: overlap mass ≥ 0.5 covers a small fraction of speech time — cost
   is bounded and single-voice behavior is untouched by construction.
3. **Votes, not verdicts**: separated-stream embeddings enter the existing
   margin-gated vote channels (wall-atom + sub-turn). The render and its
   ear laws stay authoritative; a separator hallucination can at worst add a
   vote, never force a label.
4. **Mixture abstains inside overlap**: where separation ran, the whole-span
   mixture embedding's vote is dropped for atoms substantially inside the
   overlap span (it is the failure mode being fixed). Outside, unchanged.
5. **Two streams out, identity by embedding**: the separator returns
   unordered streams; attribution is entirely the existing ref-anchored
   cosine vote. The separator never sees or emits speaker identity.

## Security model

- The ONNX model is an untrusted input: hash-pinned in `model_download`,
  loaded through the same ort sandbox path as pyannote/TitaNet; no new file
  I/O paths; transcript/audio data does not leave the device (local
  inference only, no telemetry, unchanged).

## Adversarial tests (RED before GREEN)

1. **Synthetic two-voice mixture** (pure port test, no real model): mix two
   known fixture speaker spans at overlapping offsets; the fake separator
   returns the two clean streams; the vote assembly must attribute each
   stream's atoms to the correct voice — RED today (mixture vote buries the
   quieter voice), GREEN with the pre-pass.
2. **Separator garbage**: adapter fed white noise returns streams whose
   votes fail the margin bar → assembly abstains; no crash, no label change.
3. **Model missing**: port load fails → `commands.rs` logs and runs the
   existing pipeline byte-identically (gate must stay 18/18 with S7c amended).
4. **Overlap false positive**: pyannote overlap mass fires but only one voice
   is present (measured class confusion) → separated streams' votes must not
   flip a decided single-voice label (margin bar + mass threshold).
5. **In-domain quality gate** (before model commitment): run the candidate
   checkpoint on the real cde5c264 overlap spans; the separated "Gotcha."-tail
   stream must score closer to UserA's reference than the mixture does, and
   S7c's atom must rescore decisive. If no candidate clears, the change stops
   at this task with the measurement recorded — no integration.
6. **Gate graduation**: once live render passes, remove
   `S7c_idontknow_userA` from `known_limitations` (entry + pin become hard);
   the gate fails if the label regresses.

## Test commands

`cargo test --lib audio::speaker` (port + assembly unit tests),
`cargo test --release --test overlap_separation_probe` (new offline probe:
real model on fixture spans), full `ear_truth_gate` replay, live
`live_speakers_run` persist, fresh row dump for the user's ear.

## Addendum (2026-09-25, S16 attested overlap)

The S16 case showed votes alone cannot repair an overlap region: whisper's text
itself is corrupted (voices interleaved out of order, echo copies with zero
walls). Scope extension: where separation runs on an attested-grade overlap
span, the separated streams are ALSO re-decoded (existing whisper path, stream
by stream) and the atoms re-timed by the streams' own diarization — replacing
the interleaved text with per-voice text in true order. Honest-evidence guard:
only spans where pyannote's overlap mass fires AND the mixture text shows the
degenerate signature (zero-wall duplicates, order inversions) are re-decoded;
the in-domain quality gate (task 2.2) still decides whether any of this ships.


## As-built amendments (2026-09-28, measured by the quality probe)

- **Context padding is load-bearing**: a bare span (0.4 s) separates weakly;
  the adapter infers over span ± `SEPARATION_CONTEXT_SECS` (1.3 s) and carves
  the span from the streams (probe c1: 0.342 with context vs 0.172 without).
- **RMS normalization is load-bearing**: the export's stream gain is
  thousands × the input and TitaNet is log-compressed; each carved stream is
  normalized to the input clip's RMS before any embedding.
- **Hosting**: the exported artifact ships IN-REPO at `frontend/models/`
  (public model, Asteroid MIT / LibriSpeech CC-BY-4.0); the suite enforces
  the sha256 pin on the committed copy.
- **Sequel**: at the S16 crosstalk both streams are decisively different
  voices on the same walls — one atom = one badge cannot carry simultaneous
  speech, and the order-scramble lives in the TEXT. The render-level fix is
  separated-stream re-transcription (the follow-up change this pre-pass
  enables); S16 stays the amended limitation until then.
