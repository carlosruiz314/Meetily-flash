## 1. Wall carriage through alignment (red-first, plain `cargo test`)

- [x] 1.1 RED: every aligned segment produced from token-walled source
      words carries the walls of exactly its own text's alphanumeric
      words (`Option<Vec<TokenWord>>` on `AlignedSegment`, populated from
      the `UnitWord`s at segment formation; empty/None for token-less
      segments).
- [x] 1.2 RED: consolidation merge concatenates the merged rows' wall
      lists verbatim in row order (no re-stamp, no re-derive); the
      merged row's text is unchanged.
- [x] 1.3 RED: duplicate-cluster resolution keeps the survivor's walls
      with its text; absorbed members' walls vanish with their text.
- [ ] COMMIT CHECKPOINT.

## 2. Persist + invariants

- [x] 2.1 RED: regeneration writes the segment's wall JSON into the
      existing `token_timestamps` column; a token-less segment persists
      NULL and the text is byte-identical. [DONE 2026-11-19:
      persist_regenerated_rendering binds the segment's walls, NULL when
      wall-less; red test pins both cases + text identity.]
- [x] 2.2 RED: wall integrity property — every walled row's tokens sit
      inside the row's own bounds, starts monotone non-decreasing, and
      the list's alphanumeric words equal the row text's in order.
      [DONE 2026-11-19: proptest over generated token-walled shapes
      (256 cases, census_replay_harness); reference-meeting shapes
      verified live in 3.2.]
- [x] 2.3 RED: run-over-run stability — same source + same diarization
      ⇒ same (text, span, badge, token-wall) multiset; re-verify the
      explore enumeration at apply time. [DONE 2026-11-19: determinism
      pinned (wall_stability_same_input_same_walls); enumeration
      re-verified with one CORRECTION — the lib-level
      regeneration_invariants_property DID pin the render column NULL
      (the superseded doctrine encoded in src, invisible to the
      tests/-only sweep); it is updated to the new wall-integrity
      invariant rather than deleted.]
- [x] 2.4 RED: repair-spliced synth rows persist NULL (unchanged) — the
      paused change owns that surface. [DONE 2026-11-19:
      synth_rows_carry_no_walls pins the splice's stream rows NULL;
      synth_rows_for_streams constructs None at birth.]
- [ ] COMMIT CHECKPOINT.

## 3. Verification + handoff

- [x] 3.1 Full `cargo test` green at `-j 2` (no concurrent cargo during
      live runs). [DONE 2026-11-19: all targets ok, 0 failed — lib 801
      passed / 20 ignored.]
- [x] 3.2 One production Speakers re-run (heartbeat): text byte-identical
      to the pre-change render for every row; the run log records a
      wall-integrity census line; `transcript_sources` byte-identity.
      [DONE 2026-11-19: two runs, both 0 text/span/badge/synth diffs over
      269 rows vs the pre-change render; sources sha256 unchanged. Run 1
      census rows=269 walled=194 null=75 violations=119 — RED: traced to
      merge_same_label_fragments dropping merged fragments' walls (fixed,
      389f3dba). Run 2 violations=2 — the detokenize-reflow case
      (consolidation joins two wall words into one text word; joined
      content identical); census comparator now applies the same reflow
      to both sides (the ear gate's own semantics), red-first pinned;
      local replica on the live DB: violations=0.]
- [x] 3.3 Smoke-spec deliverable decision (AGENTS.md §3, made at
      proposal time): no new frontend wiring — the visible outcome
      (rows, text, badges) is unchanged and pinned by the existing
      Speakers smoke specs; the token column is engine-side and
      structurally invisible to Playwright. No new smoke spec; rationale
      recorded. [VERIFIED 2026-11-19: both covering specs pass —
      diarization-speaker-split-persistence + diarization-label-quality,
      2 passed / 0 failed.]
- [x] 3.4 Handoff: re-run the paused `talkover-word-order-repair`
      probe's baseline check on the new render — the user's go then
      resumes its §3 attested-route wiring with the token-split straddle
      policy. [DONE 2026-11-19: handoff-note.md — baseline pins hold
      (S16 2/2, S18 7/7), the S16 mega-row is wall-bearing, and a
      measured re-cut [1024.75,1056.50] (~31.75s ≤ cap, west edge
      row-aligned, east edge token-aligned at 1056.50) covers both
      needles with no erase; probe record sha256 6bc84ab0…]
- [ ] 3.5 Before archive: re-read the live spec + design against what
      landed; amend the delta first if the implementation evolved.
- [ ] 3.6 `/opsx:archive` on the user's go (commits and pushes only on
      the user's explicit word).
