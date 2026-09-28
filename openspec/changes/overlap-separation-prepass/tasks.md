# Tasks: overlap-separation-prepass

Ordered adversarial-TDD: each task writes the failing test first. No hardcoded
overrides anywhere — the render only ever changes through evidence channels.

## Phase 1 — port + assembly (no real model)

- [x] 1.1 RED: `ports/voice_separation.rs` trait + fake separator unit test —
      synthetic two-voice mixture (two fixture speaker spans summed with an
      offset) must attribute each stream's atoms to the correct voice through
      the vote assembly. Today's mixture-only path buries the quieter voice
      (this is the S7c defect shape).
- [x] 1.2 GREEN: vote assembly consumes separated-stream votes: per-stream
      wall-atom votes (reuse `token_wall_atoms`) into the existing
      `voice_votes` channel; mixture votes dropped for atoms substantially
      inside a separation span. Unit tests: garbage streams abstain
      (margin bar), empty streams no-op, single-voice spans untouched.
- [x] 1.3 `run_engine` overlap-span detection from `FrameMasses.overlap`
      (const thresholds `OVERLAP_TRIGGER_MASS`, `OVERLAP_MIN_SPAN_SECS`),
      port invoked only there; missing port / failed call degrades silently
      (logged) — unit test asserts byte-identical output without the port.

## Phase 2 — model (stop here if the quality gate fails)

- [x] 2.1 Research + pick the ONNX checkpoint: Asteroid Conv-TasNet 16 kHz
      2-speaker family exported via torch.onnx (SepFormer rejected: CPU cost).
      Deliverable: exported artifact + sha256 + export script in
      `openspec/changes/overlap-separation-prepass/tools/`.
      DONE 2026-09-28: checkpoint `JorisCos/ConvTasNet_Libri2Mix_sepnoisy_16k`
      (noisy-mixture variant, closest to meeting conditions) exported with a
      DYNAMIC time axis via tools/export_conv_tasnet.py; artifact
      conv_tasnet_libri2mix_sepnoisy_16k.onnx (20.2 MB) in ~/.meetily-models/,
      sha256 ed1f7aeeb6c90b20ea78178468393aa7502c406d6fde893ce27145fec1cb2d29,
      graph I/O mix → est_source (1,2,T); ORT CPU round-trip verified at 2 s
      and 5 s (output preserves input length).
- [x] 2.2 STOP-GATE PASSED (2026-09-28, probe
      tests/overlap_separation_quality_probe.rs, MEETIFY_LIVE_DIAG-gated;
      audio from the local recording folder, resolved at runtime — nothing
      meeting-derived in the repo). Measured, after two probe-geometry
      corrections (RMS normalization — the export's stream gain is
      thousands × input; atom carving from context streams):
      c1: the stream carved from the 3 s neighborhood beats the mixture on
      the userA reference — cosine 0.342 vs 0.205 (bare 0.4 s span FAILS at
      0.172 → the adapter infers over span ± 1.3 s context and carves);
      c2: the S7c atom rescores in the userA-identified stream at
      userA=0.378 vs 0.087 next — margin 0.29 (bar 0.05); span-level stream
      identities are crisp (userB 0.49 / userA 0.44);
      c3: the S16 crosstalk resolves into exactly two distinct voices —
      userC margin 0.135, userA margin 0.147.
      Two probe iterations were needed; the first run failed c1 (raw
      geometry: no normalization, no context, whole-stream embedding for
      c2).
- [x] 2.3 Adapter `separation/conv_tasnet.rs` implementing the port over the
      artifact (windowed inference, 16 kHz in/out), `model_download`
      registration (hash-pinned), commands.rs wiring with graceful
      degradation. RED: model-missing test (gate output byte-identical).
      DONE 2026-09-28: adapter infers over span ± SEPARATION_CONTEXT_SECS
      (1.3 s), carves the span, RMS-normalizes to the input clip; missing
      model → from_models_dir() None (warned) → mixture-only channel
      (byte-identical, pinned by merge_without_separation test). The
      exported artifact is HOSTED IN-REPO (user's call, 2026-09-28): a
      public model (Asteroid MIT / LibriSpeech CC-BY-4.0) committed at
      frontend/models/conv_tasnet_libri2mix_sepnoisy_16k.onnx — no release
      asset, no download URL, fresh clones provision with zero manual steps.
      separation_model_path() prefers the runtime models dir (newer manual
      export wins) and falls back to the committed copy; the suite enforces
      the sha256 pin on the committed copy
      (committed_separation_model_resolves_and_matches_pin).

## Phase 3 — prove it on the ear

- [ ] 3.1 Offline gate: all 18 entries pass; the S7c render-text pin flips to
      OK on the replay; then graduate — remove S7c from
      `known_limitations`, making it a hard pin. Census rerun: DISAGREE/MIXED
      must not regress.
- [ ] 3.2 Live `live_speakers_run` persist; fresh full row dump; the
      33.17–38.46 window must read: "Yeah."→UserB, "Gotcha."→UserA,
      "Where is UserC?"→UserB, **"I don't know."→UserA**,
      "Let me ping him."→UserA. User's ear confirms.
- [ ] 3.3 Surface note: this change adds no UI surface; the label flow is
      pinned by the offline ear-truth gate + persisted-row dumps (the
      Speakers run itself is too heavy for an E2E smoke spec — same rationale
      as subturn-voice-attribution). Recorded per §3 of AGENTS.md.
