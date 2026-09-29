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

- [ ] 1.1 RED: `synthesize_overlap_rows` — covered span + both-stream
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
- [ ] 1.2 GREEN: pure function + text guards (reuse degenerate-repeat dedup
      and the language pin on stream text; trimmed non-empty check —
      whitespace/punctuation-only text does not synthesize); adversarial
      suite green.

## Phase 2 — transcription seam

- [ ] 2.1 `commands.rs` builds the closure over the EXISTING
      `whisper_engine::commands::WHISPER_ENGINE` static (no confidence
      floor — design thread B: the returned confidence is a text-length
      proxy, anti-correlated with hallucination) and passes it as
      `&dyn Fn(&[f32]) -> Option<String>` into the synthesis pass.
      Panel-corrected split: per-span synthesized CANDIDATES (separation +
      decode + per-stream vote as `{stream → (cluster, margin, text)}` on
      the extended vote record) are computed INSIDE the existing
      commands.rs:645 spawn_blocking; the PURE splice runs after
      `resolve_duplicate_clusters`. New sync decode method on
      WhisperEngine (blocking_read over current_context; stats locks
      skipped/migrated), unit-tested on a plain thread. Language resolved
      ONCE per run (concrete code or the mixture rows' resolution; never
      auto-translate, never per-stream detect). Deterministic stream
      profile: greedy, temperature 0, no token timestamps. RED: language
      reaches the decode (fake engine records the arg); full run under
      `#[tokio::test]` does not panic.
- [ ] 2.2 RED: whisper unavailable / separation model missing → render
      byte-identical to today's, verified by the structural signature
      comparator (count/text/span/badge, generated ids excepted) —
      "existing pin" covered only vote-level; this pins the render level.

## Phase 3 — render + persistence

- [ ] 3.1 RED: immutability — full run leaves `transcript_sources`
      byte-identical (source_hash digest comparator); regenerated
      rendering contains the stream rows.
- [ ] 3.2 GREEN: stream rows persist with `speaker_source` `'auto'`
      (persist hardcodes it; a new SpeakerSource variant is out of scope
      for v1 — panel); stream rows carry `continues_previous = true` when
      their text begins mid-sentence (hard-invariant compliance);
      persistence round-trip (persist → refetch → badges and text intact);
      consolidation treats stream rows as atoms (re-run does not re-merge
      a stream row into a same-speaker neighbor).
- [ ] 3.3 Gate extension: gate constructs a real Whisper engine (degrade
      would suppress every synthesized row — panel) and asserts the S16
      window renders two distinct-badge in-order rows (needles from the
      local fixture; conditional on 0.1 confirming S16 fires); duplicate
      scan's exemption provenance-scoped at GROUP level (intersecting
      absorbed-row id sets pass, disjoint-id overlaps still fail — design
      thread A, adversarial test 9); census over ALL trigger-fired spans
      (walls, trigger mass, per-stream margins incl. rejects, RMS ratios,
      both voices, token-only text data — counts + sha256, word-loss and
      cross-stream flags; adversarial test 17), recorded via a named
      runner script with token-only output; `openspec/changes/**/gate-
      runs/` added to .gitignore and the pre-push guard's protected
      pathspecs; every synthesized row's span covered by an overlap span
      (mechanical no-outside-synthesis assertion, synthetic-subset
      assertable).
- [ ] 3.4 Live `live_speakers_run`-equivalent persist; DB-layer verification
      (terminal-only rule): S16 rows per-voice, transcript_sources
      untouched.
- [ ] 3.5 Re-pin the gate's render snapshot AFTER 3.4 (explore finding 4).
      Panel semantics addition: the re-pinned snapshot is the
      post-synthesis render — verify replay idempotence (synthesis on
      already-synthesized input must not double-synthesize; word-loss
      baseline documented as lost after re-pin, so census baselines come
      from the 0.1/3.3 runs) and name the re-pin tool's current location
      (it lived in the archived no-split-sentences change).
- [ ] 3.6 Playwright smoke spec extending `e2e/smoke/speaker-
      diarization.spec.ts` (§3 deliverable, recorded here per AGENTS.md):
      mutate the mock's command-handler fixture to return a stream pair
      for one span, assert the post-refetch render shows two badges at
      the same timestamp in order, and no undo affordance on auto rows.

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
