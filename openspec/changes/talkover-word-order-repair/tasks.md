## 1. Candidate census — measure the flagging before touching anything

- [ ] 1.1 Env-gated census probe (`#[ignore]`, `MEETIFY_LIVE_DIAG`, loud
      evidence-home skip, token-only): enumerate every overlap-span
      candidate flag on the reference meeting with its detector evidence —
      overlap mass, wall duration, floors verdict (fires / stood-down),
      sorted by position; record counts + sha256 only; verbatim never
      leaves the terminal without `MEETIFY_RENDER_PRINT`.
- [ ] 1.2 Review the recorded census with the user (STOP gate): how many
      candidates fire, how the flag rate compares to attested talk-overs,
      whether detector precision needs a dial conversation. No promotion
      wiring before this review.

## 2. Decode reliability — offline, red-first

- [ ] 2.1 RED (plain `cargo test`, invented generic tokens): the retry
      profile — deterministic greedy/temperature-0 first pass; a
      no-condition retry fires only when the first pass trips rate-fiction
      or a stutter/hallucination guard; retry is attempted at most once;
      both passes' evidence lands in the decode record.
- [ ] 2.2 RED: repair-window decode scoping — a normal transcription chunk's
      profile, walls, and output are byte-identical to the pre-change
      pipeline (the retry profile is structurally unreachable outside
      repair windows).
- [ ] 2.3 RED: widened-window rebuild — a promoting seed's window rebuilds
      at the evidence-backed context width by the existing repair-window
      rules; two seeds in one window union deterministically; the window
      never crosses a manual-row claim.
- [ ] 2.4 Offline probe on the three kept window shapes (env-gated,
      evidence-home fixtures, token-only record): widened window + retry
      profile vs the attested sequences. GREEN bar: each attested shape
      recovers verbatim. Any unrecovered shape = recorded STOP (user
      re-plans; no wiring ships).

## 3. Promotion wiring — rebuild from the archived sketches

- [ ] 3.1 RED: seed = a rate-fiction/stutter span that REACHED separation
      (duration-floor carve-out excluded); non-seeds route nowhere; the
      promotion is unreachable from the repair fold until wired (negated
      reachability test first).
- [ ] 3.2 GREEN: route the promoted window through the live repair-window
      builder (separation + per-stream decode + per-voice splice with
      `synth_parent` links); all existing rejection guards apply unchanged
      (margin-gated distinct-voice votes, hallucination guards, RMS
      eligibility, duration floor, w/s ceilings, manual-row-wins,
      byte-identical degrade, `transcript_sources` byte-identity).
- [ ] 3.3 RED→GREEN: the attestation gate — `attestedRepairWindows`
      (token-only `{start_s, end_s, attested_date}`): exact-wall equality
      required; unpinned fired span keeps the mixture render
      (byte-identical) and records `awaiting-attestation`; pinned + guard-
      clean window splices; no wildcard; production code never reads the
      evidence fixture (structural test).
- [ ] 3.4 RED→GREEN: census outcomes for every seed — `fired+attested`,
      `awaiting-attestation`, `stood-down:<guard>`, `window-not-built`,
      `separation-unavailable` (token-only record shape, count-keyed).

## 4. Ear-gate auditability

- [ ] 4.1 RED→GREEN (plain `cargo test`, token-only): ear-gate cross-check —
      an allowlist entry with no matching fixture pin fails the gate;
      a per-voice rendered span with no allowlist entry fails the gate;
      overlapping-but-unequal walls are treated as unpinned; entries carry
      non-empty YYYY-MM-DD dates (completeness lint, waiver-router
      conventions).

## 5. Verification + closure

- [ ] 5.1 Full `cargo test` green at `-j 2` including the ear-gate fixture
      lint (no concurrent cargo during live runs).
- [ ] 5.2 Production Speakers re-run (heartbeat): verify via the
      replay/oracle harnesses — fired spans render per-voice ONLY where
      attested; every seed outcome present in the census; unpinned spans
      byte-identical to mixture rows; `transcript_sources` byte-identity.
- [ ] 5.3 Attestation round: cut before/after clips ONLY for fired spans
      (evidence home or meeting folder), token-only manifest; one
      confirmation listen per span; dated verdicts become fixture pins +
      allowlist entries (user's word each time). Spans stood down on
      verdict keep mixture rows permanently.
- [ ] 5.4 Smoke-spec deliverable decision (AGENTS.md §3, made at proposal
      time): the promotion changes WHICH rows the engine derives, not the
      render surface — the event→refetch→render wiring stays pinned by
      `frontend/e2e/smoke/diarization-speaker-split-persistence.spec.ts`;
      the engine-side selection is verified by the cargo gates + ear gate
      pins. Record the rationale; verify the covering specs still pass.
- [ ] 5.5 Before archive: re-read the live spec + design against what
      landed; amend the delta/design first if the implementation evolved.
- [ ] 5.6 `/opsx:archive` on the user's go (commits and pushes only on the
      user's explicit word).
