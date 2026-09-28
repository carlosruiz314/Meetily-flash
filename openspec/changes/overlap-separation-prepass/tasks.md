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

- [ ] 2.1 Research + pick the ONNX checkpoint: Asteroid Conv-TasNet 16 kHz
      2-speaker family exported via torch.onnx (SepFormer rejected: CPU cost).
      Deliverable: exported artifact + sha256 + export script in
      `openspec/changes/overlap-separation-prepass/tools/`.
- [ ] 2.2 RED: in-domain quality probe (offline, cde5c264 real audio) —
      over the measured overlap spans (incl. 34.5–34.9 @ 0.83 mass and the
      S7c atom neighborhood): the separated stream closest to UserA's
      reference must beat the mixture cosine, and the S7c atom must rescore
      decisive (margin ≥ 0.05). Record numbers in this file. If no candidate
      clears: STOP, record, keep S7c amended.
- [ ] 2.3 Adapter `separation/conv_tasnet.rs` implementing the port over the
      artifact (windowed inference, 16 kHz in/out), `model_download`
      registration (hash-pinned), commands.rs wiring with graceful
      degradation. RED: model-missing test (gate output byte-identical).

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
