# Tasks: sub-run voice-flip scan

## 1. Evidence

- [x] 1.1 Offline probe: pure boundary diagnostics at the ear pins
      (`tests/engine_offline_probe.rs::boundary_diagnostics_at_ear_pins`) —
      pin B has NO candidates (label-change and trust-zone slot-change both
      empty near 12.0s); the run [9.38,13.03] carries one piece.
- [x] 1.2 Raw-mass dump proves the decode holds sp1 argmax through the
      back-channel (9.4–11.95: 0.94–1.00; 12.03–12.71: mush, argmax kept).
- [x] 1.3 TitaNet separation scan
      (`embedding_separation_at_pin_b`): same-voice 0.31–0.52 vs cross-voice
      0.055; dip minimum 11.6–12.7s. Cached decoded samples
      (`samples_16k.f32` beside the audio) end the ~6.5-min per-iteration
      decode.
- [x] 1.4 Valley census: 639 valley spans / 162.9s contested meeting-wide;
      pin B valley [11.998, 12.825] — valley start = the pin (12.0).

## 2. Engine

- [x] 2.1 `confusion_valleys` in `run_assembly` + 4 unit tests
      (pin-B signature, clean/solo-quiet frames, near-valley merge, run
      containment).
- [x] 2.2 Flip scan in `derive_turns_from_masses`: valley-gated, window
      1.2s, decisive-different-centroid acceptance (margin ≥
      AMBIGUITY_MARGIN both sides), bounded rounds, deterministic splice.
- [x] 2.3 Samples cache wired into `ear_truth_gate` (identical bytes to
      `to_whisper_format()`; meta pins the sample count).

## 3. Gate

- [x] 3.1 Gate replay with the scan: FLIP-SPLIT at 12.00 (sp1→sp0, margins
      0.528/0.064); 2 further flips (2.73s, 1026.73s); 193 abstentions
      (same-cluster and undecided valleys); zero flip decisions in the
      Ricardo window (30–46s). 16/16 entries PASS, 0 fractures of 180 rows.
      Evidence: `openspec/exploration/bothbars-flipscan-gate-green-20260909.log`
- [x] 3.2 `S2_user_to_cynthia_13s` promoted: removed from
      known_limitations + amendment record removed; note records the fix.
      Gate re-run green with S2 ASSERTED (exit 0).
- [x] 3.3 Driven persist runs → re-pin → gate green. Persist run 1
      (173-row DB state): 178 rows, "Yeah. That's right." correctly its own
      row under the user's badge. ROOT CAUSE of the remaining Ricardo
      misbadge found: the pipeline consumes its own aligned output, so
      sentence punctuation was already destroyed ("Where is Ricardo I don't
      know." — unfixable by any re-run; filed as
      `align-from-immutable-source`). Restored the TRUE source
      (transcripts.json, 237 rows) and re-ran: "Yeah. That's right." holds;
      the question renders under Cynthia; 0 fractures meeting-wide.
      Re-pinned snapshot (173 rows, sha 13a9ddf2…). Final gate: 16/16
      asserted-pass, 0 known-limitations, 0 fractures of 181 rows, exit 0
      (`bothbars-final-gate-green-20260909.log`). The gate's unmerged check
      moved to its production stage (pre-dedupe): the dedupe absorption can
      make same-row fragments adjacent, a benign shape the persisted row
      presents as one row.
- [ ] 3.4 USER EAR CHECK of the persisted UI rows (both bars), then commit
      on explicit permission.

## 4. Badge accuracy (both-bars follow-up, same session)

- [x] 4.1 Root cause of wrong-badge rows: the greedy online clustering
      left CARLOS with no centroid (his anchors scored 0.12–0.32 against
      both surviving centroids — Cynthia 0.72, Ricardo 0.71) — his pieces
      flipped between clusters by thin margins and Ricardo's real speech
      shared his badge (2801–2821 rendered under Carlos).
- [x] 4.2 `cluster_pieces_ahc` (average-linkage agglomerative, same
      threshold semantics, deterministic) replaces the greedy pass in the
      engine; 5 unit tests. AHC alone does NOT fix Carlos (his pieces
      don't self-cluster at 0.65) but measures the matrix directly and no
      longer depends on processing order.
- [x] 4.3 Voice fingerprinting (enrollment): seeded the stamped pool with
      ear-attested anchor-mean embeddings for Carlos (S2 "Yeah" 12.0–12.8 +
      pre-join 42.2–50.0) and Cynthia (S1 9.75–11.8 + 35.0–36.0) —
      reversible rows `emb-earseed-carlos-…` / `emb-earseed-cynthia-…`.
      Engine refs path resolves all THREE voices: centroids 0=Carlos
      (0.35–0.52), 1=Cynthia (0.72), 2=Ricardo (0.76).
- [x] 4.4 Persist from true source with refs: Ricardo's stretch renders
      under his own badge (Speaker 2, 55 rows), Carlos consistent (39
      rows), Cynthia named (88 rows). Gate 16/16 asserted, 0 fractures of
      184 rows, snapshot re-pinned (182 rows, af1e2465…). Evidence:
      `identity-refs-gate-green-20260909.log`,
      `identity-final-gate-green-20260909.log`,
      `identity-persist-run-20260909.log`.
- [ ] 4.5 Known residual (token-less rows): "I don't know." / "I can't."
      atoms inside the 32.51–40.24 source row take Cynthia's badge via
      skewed proportional word timing — needs word-level timestamps or
      skew-tolerant atom assignment (separate change).
