# Tasks: overlap-stream-retranscription

Ordered adversarial-TDD: each task writes the failing test first. No
hardcoded overrides; audio from the local recording folder resolved at
runtime (env override, marker literal never in code); token-only text in
committed tests AND in every recorded artifact (census, gate-runs logs,
clip manifests — verbatim meeting text is terminal-only or in the private
evidence home, never repo-committed).

## Phase 0 — blocking measurement (pre-implementation)

- [x] 0.1 Dry-run census probe (env-gated, existing pipeline, no render
      change): log every trigger-fired span — trigger mass, per-stream
      best/second similarities and margins including REJECTS, energy
      split — plus S16's per-frame mass profile at 1055.5–1057.5 and the
      total span count. TOKEN-ONLY output. OUTCOME (2026-09-30): S16 does
      NOT fire — mass peaks 0.726 (bar 0.5 is crossed) but never sustains
      0.4 s (longest run 0.270 s); dial sweep recorded in design.md,
      decision with the user. Probe:
      `frontend/src-tauri/tests/overlap_census_probe.rs`.

## Phase 1 — synthesis core (pure, fakes only)

- [x] 1.0 Trigger dial (user-ruled 2026-09-30, task 0.1 sweep):
      `OVERLAP_MIN_SPAN_SECS` 0.4 → 0.25 (mass bar 0.5 unchanged). RED:
      the boundary unit test pins 0.25 exactly — a run sustaining ≥0.5
      mass for 0.24 s does NOT fire, 0.25 s does; the attested S16 window
      (longest 0.5-bar run 0.270 s) therefore fires.
- [x] 1.1 RED: `synthesize_overlap_rows` — covered span + both-stream
      decisive votes (per-span record carrying stream identity, margins,
      and texts) + per-stream texts → per-voice rows replacing the span's
      mixture rows, traced to source row ids. Adversarial: silent stream
      → no synthesis; hallucinated echo text → guard drops it; margin
      miss on either stream → no synthesis; same-badge collapse → no
      synthesis; edge-clamped span → no panic; oversized span → guard or
      degrade; stream row with an original_id absent from the source set
      → rejected (persist would silently drop it — explore finding 2);
      multi-row span → pinned max-covered-atom id selection. (Checkbox
      reset by the panel: only this RED spec existed, no test — apply
      starts here.)
- [x] 1.2 GREEN: pure function + text guards (reuse degenerate-repeat dedup
      and the language pin on stream text; trimmed non-empty check —
      whitespace/punctuation-only text does not synthesize); adversarial
      suite green. (11 adversarial tests + full run_assembly suite green;
      full lib 726/726 twice — one unrelated first-run flake did not
      reproduce.)

## Phase 2 — transcription seam

- [x] 2.1 `commands.rs` builds the closure over the EXISTING
      `whisper_engine::commands::WHISPER_ENGINE` static (no confidence
      floor — design thread B: the returned confidence is a text-length
      proxy, anti-correlated with hallucination) and passes it as
      `&dyn Fn(&[f32]) -> Option<String>` into the synthesis pass.
      DONE (2026-09-30): `StreamDecoder` (engine from the static +
      `resolve_stream_language`: concrete code pins, automatic states
      DEGRADE — never auto-translate; delta spec aligned) wraps
      `WhisperEngine::transcribe_span_blocking` (sync strict profile,
      `blocking_read`, stats locks skipped; spawn_blocking-safety pinned
      by `#[tokio::test]`); per-span candidates
      (`SpanSynthesisInput`: span-level identity, decode-ready streams,
      covered atoms, pre-normalization RMS ratios via the new
      `SeparatedStream.pre_rms_ratio`) computed INSIDE the existing
      spawn_blocking; pure splice after `resolve_duplicate_clusters` with
      `list_manual_spans` stand-down; hallucination audit in the decoder.
      Eligibility floor `SYNTH_STREAM_MIN_RMS_RATIO = 0.1` implemented
      (panel finding 7). Lib 732/732.
- [x] 2.2 RED: whisper unavailable / separation model missing → render
      byte-identical to today's. DONE (2026-09-30): the splice is extracted
      as `apply_overlap_synthesis` and pinned — no candidates or an
      all-degraded span returns the input unchanged; the persist layer's
      structural signature comparator ((text, span, badge) multiset, ids
      excepted) pins render identity; double-run idempotency already
      pinned.

## Phase 3 — render + persistence

- [x] 3.1 DONE: immutability pinned by
      `synth_rows_persist_with_facts_and_sources_stay_immutable` —
      transcript_sources rows compared before/after a persist that
      synthesized stream rows (stronger than a digest: full-row
      comparison).
- [x] 3.2 DONE: stream rows persist as `'auto'` with a new nullable
      `synth_atom` marker (migration 20260930000000) — consolidation
      isolates synth rows via the manual-row singleton-group pattern
      (`consolidation_never_merges_a_synth_row_into_a_same_speaker_neighbor`,
      with a control pair proving plain rows still merge, and idempotent
      re-run); synth rows carry `continues_previous` from a mid-sentence
      check at INSERT (engine stamping preserves it via OR semantics);
      round-trip asserted (badges/text/facts intact after refetch).
      Projection drift detector extended (synth_atom +
      continues_previous = override cols). Lib 735/735.
- [x] 3.3 DONE (2026-09-30): gate constructs a real Whisper engine
      (MEETIFY_GATE_WHISPER_MODEL; MEETIFY_GATE_LANG pins the decode —
      unset = degrade, never auto-translate) and replays the SAME pure
      splice after the resolver via `apply_overlap_synthesis` (now pub);
      S16 assertion — two distinct badges + per-voice needles from the
      LOCAL fixture (`needles` field, serde-defaulted) — routed through
      the amendment path (AMENDED while the waiver stands; 4.2 removes
      it); duplicate + fracture scans exempt provenance-scoped pairs at
      GROUP level (synth rows sharing one source id; disjoint ids still
      fail — adversarial tests 9/13); token-only census per candidate span
      (walls, span-in-spans guard, identities, margins, RMS ratios, word
      counts, sha256 of both texts; verbatim text only under
      MEETIFY_RENDER_PRINT, terminal-only); runner script
      `tools/run_ear_gate.bat` records output into `gate-runs/`
      (gitignored + pre-push pathspec Gate 1). Gate lint + lib 735/735.
      LIVE GATE GREEN (2026-09-30, second replay at the ruled semantics):
      132 candidates at mass 0.3, 10 synthesized, S16 window renders 3 rows
      under 2 distinct badges with 2/2 fixture needles — full gate pass on
      the real meeting.

- [x] 3.4 DONE (2026-09-30): `live_speakers_run_cde5c264` green in 5190s
      against the real meeting DB — 3 speakers, 442 segments, unmatched=[];
      transcript_sources byte-identical (hard assert, pre/post hash);
      10/132 spans synthesized per-voice (matches the gate); S16 window
      (1054–1059s) holds 2 synth rows under distinct badges ("Speaker 2"
      "I don't know if I can't do it." / "Participant A" "I can't wait to
      see it."). Harness fixes that made it land: sqlx::migrate! before
      the run, WHISPER_ENGINE static populated from the production model
      store + discover_models.
- [x] 3.5 DONE (2026-09-30): re-pin resolved to a NO-OP, verified not
      assumed. Design finding 4 ("snapshot must be re-pinned after the
      live persist") predates the align-from-immutable-source pivot: the
      gate's cross-check target is `transcript_sources` (commands.rs:1492
      is also the production synthesis input), which never contains synth
      output. Direct hash check post-persist: fixture sha ==
      live transcript_sources sha (9b643fda…, 229 rows) — no drift;
      word-loss baselines from 0.1/3.3 remain valid (nothing re-pinned).
      Replay idempotence is structural: every Speakers run re-derives
      from the immutable source table and rewrites the rendering wholesale
      (locked by `regeneration_rebuilds_consolidation_shaped_rendering`);
      synthesis on already-synthesized input cannot occur. Live rendering
      verified: 20 synth rows (10 spans × 2 streams) + 32 rows with
      synth_parent (20 stream + 12 split head/tail). Re-pin tool location
      named and corrected in the gate's drift-panic message:
      `openspec/changes/archive/2026-09-13-no-split-sentences/tools/
      snapshot_fixture.py` (was missing the archive prefix).
- [x] 3.6 DONE (2026-09-30): smoke 15.3e in `e2e/smoke/speaker-
      diarization.spec.ts` — the new persisted shape (two per-voice rows at
      identical walls) renders under its own badges and SURVIVES the
      post-run refetch (stale-render guard; the mock fixture mirrors what
      the real backend persists). 9/9 specs green on chromium.

## Phase 4 — the ear

- [ ] 4.1 Ear-calibration clip loop (user-ruled protocol, design thread C):
      the agent reads the full rerun transcript against the original and
      reports whether it makes more or less sense; then hands the user a
      clip set — word-loss-flagged moments first, remainder evenly spaced
      through the meeting, up to ~20 total — each clip with its transcript
      lines and detector evidence. Clip audio and verbatim manifests live
      in `fixture_clips/` (gitignored) or the evidence home; only
      token-only manifests are repo-visible. User's ear rules: genuine
      talk-over? lines read right? Rulings tune the detector dials as a
      general rule (never a per-moment skip list).
- [ ] 4.2 S16 graduation: on the user's confirmation, remove the S16
      waiver from the fixture's known_limitations (amendment record
      stays), making the window a hard pin.
