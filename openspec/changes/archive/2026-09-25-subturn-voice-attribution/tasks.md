# Tasks: subturn-voice-attribution

## 1. Pure split logic (adversarial TDD)

- [x] 1.1 RED: `subturn_segments` tests — lone contrary vote never splits;
      sustained run splits + re-badges; unanimous-contrary turn rebadges
      with head sliver on the original cluster; alternating sub-second
      back-channels never split; multi-change turn → 3 segments; empty
      votes keep the turn; long single chunk (≥2s) splits; abstain gaps
      don't break a run.
- [x] 1.2 GREEN: `subturn_segments` + `voiced_chunks` + consts in
      `run_assembly.rs`.
- [x] 1.3 Smoke-spec decision: engine-internal change, no new UI surface —
      covered by the ear gate + existing smoke corpus (recorded here per §3).

## 2. Engine wiring

- [x] 2.1 `subturn_voice_pass` in `run_engine.rs` after the rescue splice;
      TurnOut mapping per design; `MEETIFY_ENGINE_DEBUG` prints
      split/re-badge decisions.
- [x] 2.2 Full `cargo test` green.

## 3. Gate + census verification

- [x] 3.1 Ear gate green: 16/16 pins, 0 fractures, RENDER-TEXT
      "Oh, man" → Speaker 1.
- [x] 3.2 Live re-run persists the finer turns; DB row spot-check at the
      74s UserC/UserB exchange and the DISAGREE rows.
- [x] 3.3 Census re-runs saved to `openspec/exploration/`
      (row-voice-census-{after-subturn,final,final2}-20260922.log). Final:
      DISAGREE 6→2, MIXED 45→42; remainder is sub-second back-channels
      (TitaNet misvotes on the "Oh, man." class — kept absorbed by design)
      plus a few token-less rows.
- [x] 3.4 Full transcript text dump delivered in chat for the user's ear
      check (live-final-20260922-rows.txt; dumped 2026-09-22).

## 4. Token-word reconstruction (root cause found by this census)

- [x] 4.1 ROOT CAUSE (bigger than the pass): the token-validity clamp
      compared BPE piece count against whitespace word count — whisper-cpp
      rows ("And"+"he"+"'s") ALL failed, so 214/229 rows' real word times
      were discarded and the whole meeting rendered on proportional walls
      (±seconds error). No turn-level refinement can fix text-to-voice
      mapping on those walls; this is what made labels "constantly" wrong.
- [x] 4.2 `valid_token_words` merges pieces into whitespace words by exact
      concatenated reconstruction (mismatch → honest proportional
      fallback); zero-span clamp, cross-word ordering, output-seam
      non-overlap clip. 161/229 rows now align on real word times. Test:
      `token_pieces_merge_into_whitespace_words_with_real_spans`.
- [x] 4.3 Gate: 0 render failures, 0 invariant violations, 0 fractures,
      15/16 pins (S9 = ear question, 4.4); live run persisted 224 rows.
- [x] 4.5 Tripwires so the 4.1 collapse can never sit silent again:
      `build_logical_units` warns when ALL tokened rows fall back (the
      once-unlogged 100% case), and `tests/token_wall_tripwire.rs` replays
      the validator over the pinned fixture in seconds (no audio/DB),
      failing below the measured floor (176/229 real walls, 2026-09-24;
      the 53 fallbacks are divergent piece streams, not count rejections).
      Post-re-pin measurement; 4.2's 161/229 predates the re-transcription.
- [x] 4.6 Word-wall atom votes (the S7 fix that needed no new model): a
      shortclip probe proved TitaNet separates 0.3–0.8 s clips decisively
      AT the token walls ("Gotcha." → UserA, margin 0.46; stronger than
      1.5 s controls) while energy chunks straddling word boundaries
      out-voted it with the neighbour's mass. `run_assembly::
      token_wall_atoms` merges token words into render-atom-aligned
      pseudo-atoms (gap ≤ 450 ms, sentence terminators always close, 3 s
      cap, <250 ms drops); `run_engine::wall_atom_voice_votes` embeds them
      ref-anchored at the production margin bar; commands.rs computes them
      in the engine spawn_blocking (+2044 votes live) and the gate replays
      the same call. Gate 16/16 pins (new: Gotcha → Speaker 0), 0
      fractures; live persisted 2026-09-24: "Yeah."/"Gotcha."/"Where is
      UserC? …" are separate rows, Gotcha under UserA.
- [x] 4.8 Review-loop hardening (rounds 1-3, converged 2026-09-25):
      `wall_vote_token_streams` gates every voting row through
      `valid_token_words` (same trust boundary as the render; RED→GREEN
      test `wall_votes_flow_only_through_rows_the_render_trusts`), ref-
      collision warnings in both vote passes, wall-vote count logged at
      info. Live gate after the gate: 19 pins + 1 known-limitation (S16
      waiver) + 0 FAILED, 0 fractures. Full suite 726/0.
- [x] 4.4 RESOLVED (ear ruling 2026-09-24): "Oh, you're wearing the
      t-shirt" is USERA's — TitaNet's sustained-split vote there was
      wrong; the unsplit render under Speaker 0 conforms. Pinned twice:
      fixture entry S10_tshirt_row_userA + RENDER-TEXT 't-shirt' gate pin.
- [x] 4.7 RESOLVED (ear replay 2026-09-25: words at 36.1-36.7, DTW smear not voice confusion; fixed by turn-boundary-wall-realignment). Original record:: the 2026-09-24
      rulings — "Where is UserC?" → UserB ✓, "Let me ping him" →
      UserA ✓, "I don't know" (35.51-36.07) → UserA (INVARIANT, render
      does NOT conform yet). Three channels measure the clip
      UserB-dominated (TitaNet atom UB 0.32 / UA 0.04; sub-windows carry
      no decisive UA window; enriched UA ref still UA 0.02; pyannote s1
      mass 0.01-0.03 vs s2 0.7 solo — the 0.6-0.83 overlap mass sits over
      Gotcha's tail, not here). User directive: NO hardcoded overrides —
      algorithms only, up to maximum feasible quality. The pin is a
      declared known-limitation (S7c) with an amendment record; options
      (onset-weighted voting at the noise floor vs source-separation
      pre-pass vs accepted residual) await the user's call.
