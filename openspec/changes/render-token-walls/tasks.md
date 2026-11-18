## 1. Wall carriage through alignment (red-first, plain `cargo test`)

- [ ] 1.1 RED: every aligned segment produced from token-walled source
      words carries the walls of exactly its own text's alphanumeric
      words (`Option<Vec<TokenWord>>` on `AlignedSegment`, populated from
      the `UnitWord`s at segment formation; empty/None for token-less
      segments).
- [ ] 1.2 RED: consolidation merge concatenates the merged rows' wall
      lists verbatim in row order (no re-stamp, no re-derive); the
      merged row's text is unchanged.
- [ ] 1.3 RED: duplicate-cluster resolution keeps the survivor's walls
      with its text; absorbed members' walls vanish with their text.
- [ ] COMMIT CHECKPOINT.

## 2. Persist + invariants

- [ ] 2.1 RED: regeneration writes the segment's wall JSON into the
      existing `token_timestamps` column; a token-less segment persists
      NULL and the proportional fallback text is byte-identical to
      today's.
- [ ] 2.2 RED: wall integrity property — every walled row's tokens sit
      inside the row's own bounds, starts monotone non-decreasing, and
      the list's alphanumeric words equal the row text's in order
      (property test over generated fixtures + the reference-meeting
      shapes).
- [ ] 2.3 RED: run-over-run stability — same source + same diarization
      ⇒ same (text, span, badge, token-wall) multiset; update any
      replay/persistence-oracle harness pinning NULL walls (red-first).
- [ ] 2.4 RED: repair-spliced synth rows persist NULL (unchanged) — the
      paused change owns that surface; the legacy phrase-loop path is
      byte-identical.
- [ ] COMMIT CHECKPOINT.

## 3. Verification + handoff

- [ ] 3.1 Full `cargo test` green at `-j 2` (no concurrent cargo during
      live runs).
- [ ] 3.2 One production Speakers re-run (heartbeat): text byte-identical
      to the pre-change render for every row; the run log records a
      wall-integrity census line (walled rows, NULL rows, violations=0);
      `transcript_sources` byte-identity.
- [ ] 3.3 Smoke-spec deliverable decision (AGENTS.md §3, made at
      proposal time): no new frontend wiring — the visible outcome
      (rows, text, badges) is unchanged and pinned by the existing
      Speakers smoke specs; the token column is engine-side and
      structurally invisible to Playwright. No new smoke spec; rationale
      recorded; the covering specs verified passing.
- [ ] 3.4 Handoff: re-run the paused `talkover-word-order-repair`
      probe's baseline check on the new render (the S16 mega-row now
      carries walls; baseline pins still hold) — the user's go then
      resumes its §3 attested-route wiring with the token-split straddle
      policy.
- [ ] 3.5 Before archive: re-read the live spec + design against what
      landed; amend the delta first if the implementation evolved.
- [ ] 3.6 `/opsx:archive` on the user's go (commits and pushes only on
      the user's explicit word).
