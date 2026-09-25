# Tasks: turn-boundary-wall-realignment

- [x] 1.1 RED: in-crate integration test `s7c_dtw_smeared_atom_reanchors_past_the_voice_boundary`
      — real fixture token walls + production vote list + engine turns;
      asserts the corrected render (I don't know → Speaker 0 at [36080, 36719]).
- [x] 1.2 RED: negative tests — turn-final period, sustained_split boundary,
      no head room, same-speaker turns (all unchanged).
- [x] 1.3 GREEN: `realign_straddling_atoms` per design; call site after
      `build_logical_units`; all lib tests green.
- [x] 1.4 Gate: S7c pin hard (window [35800, 36400], Speaker 0, no waiver);
      full suite 18/18 + 0 fractures + 0 duplicates in one run.
- [x] 1.5 Live `live_speakers_run` persist; fresh row dump; 33.17–38.46
      window reads Yeah→UB / Gotcha→UA / Where is UserC?→UB /
      **I don't know→UA** / Let me ping him→UA. User's ear confirms.

All tasks complete 2026-09-25: gate 18/18 (S7c hard pin OK), live persisted 274 rows; final window: Yeah→UB / Gotcha→UA / Where is UserC?→UB / I don't know + Let me ping him→UA. Ready for /opsx:archive on the user's word (uncommitted per standing rule).

## Explore-cycle audit (2026-09-25, user-prompted)

- [x] 1.6 Audit: diffed pre/post-realignment persisted renders (271 vs 274
      rows). Found the rule over-firing meeting-wide: two false label flips
      re-anchored atoms across 0.26-0.5 s low-confidence slivers (3382.68
      UserB->UserA over a 0.5 s sp0 sliver + 11 s hole; 4147.88
      Speaker 2->UserB over a 0.26 s sliver) and two large same-label
      timing shifts (4829 +12 s, 1186 +2 s) reaching across turn ends.
- [x] 1.7 Guards (RED first, numbers from the real firings): far turn
      >= 1 s (STRADDLE_MIN_FAR_TURN_MS), head room must end inside T2
      (next_start <= t2.end), compression-only fit (scale capped at 1.0 —
      never stretch walls). New negative tests: realignment_rejects_sliver_
      far_turns, realignment_head_room_must_sit_inside_the_far_turn.
      Result: gate 18/18 (S7c hard pin OK), 0 fractures, live re-persisted
      (271 rows); guarded diff vs pre-realignment is label-identical — 15
      same-label start corrections only; both false flips and both large
      shifts reverted. Residual uncertainty (4829/1186 exact word onsets,
      whisper-walls vs pyannote-boundary) is timing-only, no label impact;
      audit clips cut for optional ear check.

- [x] 1.8 Review-loop hardening (round 2, converged round 3 2026-09-25):
      guard tests were reworked after the round-2 review showed four of
      them passed via the WRONG guard (deleting the guard would have kept
      the suite green). Each negative test now calls
      `realign_straddling_atoms` directly with the named guard as the sole
      rejector + a positive control; side-min guard pinned (design
      adversarial #2); head-room pin mutation-verified. Design
      adversarial-test section amended to match.

## §3 smoke-spec decision

No E2E smoke spec: the Speakers diarization run (~12 min, real models, real
recording) is not drivable in Playwright; the change's label flow is pinned by
the offline ear-truth gate (S7c hard pin) + persisted-row dumps — same
rationale recorded in subturn-voice-attribution tasks.md.
