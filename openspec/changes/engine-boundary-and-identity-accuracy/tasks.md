# Tasks: sub-run voice-flip scan

## 0. UI smoke-spec deliverable decision

This change is engine-internal (clustering, boundary scan, enrollment
refs) — no new user-visible surface, command arg, or event. The affected
user flow (Speakers button → diarization-complete → refetch → N distinct
labeled badges) is already pinned by
`frontend/e2e/smoke/diarization-speaker-split-persistence.spec.ts` and
`frontend/e2e/smoke/diarization-label-quality.spec.ts`; no new smoke spec
is landed (AGENTS.md §3 record-why).

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
      `to_whisper_format()`; meta pins the sample count — superseded by
      4.6: the meta now also pins the source audio's sha256).

## 3. Gate

- [x] 3.1 Gate replay with the scan: FLIP-SPLIT at 12.00 (sp1→sp0, margins
      0.528/0.064); 2 further flips (2.73s, 1026.73s); 193 abstentions
      (same-cluster and undecided valleys); zero flip decisions in the
      UserC window (30–46s). 16/16 entries PASS, 0 fractures of 180 rows.
      Evidence: `openspec/exploration/bothbars-flipscan-gate-green-20260909.log`
- [x] 3.2 `S2_user_to_userB_13s` promoted: removed from
      known_limitations + amendment record removed; note records the fix.
      Gate re-run green with S2 ASSERTED (exit 0).
- [x] 3.3 Driven persist runs → re-pin → gate green. Persist run 1
      (173-row DB state): 178 rows, "Yeah. That's right." correctly its own
      row under the user's badge. ROOT CAUSE of the remaining UserC
      misbadge found: the pipeline consumes its own aligned output, so
      sentence punctuation was already destroyed ("Where is UserC I don't
      know." — unfixable by any re-run; filed as
      `align-from-immutable-source`). Restored the TRUE source
      (transcripts.json, 237 rows) and re-ran: "Yeah. That's right." holds;
      the question renders under UserB; 0 fractures meeting-wide.
      Re-pinned snapshot (173 rows, sha 13a9ddf2…). Final gate: 16/16
      asserted-pass, 0 known-limitations, 0 fractures of 181 rows, exit 0
      (`bothbars-final-gate-green-20260909.log`). The gate's unmerged check
      moved to its production stage (pre-dedupe): the dedupe absorption can
      make same-row fragments adjacent, a benign shape the persisted row
      presents as one row.
- [x] 3.4 USER EAR CHECK of the persisted UI rows (both bars), then commit
      on explicit permission.

## 4. Badge accuracy (both-bars follow-up, same session)
      EVIDENCE 2026-11-17: Production re-run verified at the DB layer against recorded ear-truth: the S18 region carries all 6 attested phrases with correct badges (UserB's attested line at real walls [191.82-193.58]; the user's attested lines [193.58-199.50]; the attested tail [199.52-202.69]; the attested hybrid interjection in the user's stream). Five garble fixes verified mechanically (the clip-5 fluent variant; the clip-6 echo removed; the class-E sign-off hallucination gone; the clip-2 name garble resolved to the attested form; the clip-4 doubled phrase now a clean two-voice split 8.5s apart). Zero badge changes on identical rows; the floors' render conforms to the user's 2026-10-07 ruling. Persisted: 269 rows, 3 speakers, transcript_sources untouched.
