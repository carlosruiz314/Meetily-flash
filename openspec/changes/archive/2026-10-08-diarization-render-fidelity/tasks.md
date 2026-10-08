## 1. Prerequisites (design D4)

- [x] 1.1 RED: a unit test pinning the synthesis entry contract — then
      reshape `decode_span_synthesis` (commands.rs, make `pub`) to take the
      utterance-decode closure so the gate replay's two inline decode loops
      call it instead of re-implementing the decode. ONE closure (the
      span-decode variant would be dead code — the function only uses
      `decode_utterances`). GREEN: gate replay compiles down to one decode
      path; `cargo test --lib` green.
      DONE 2026-10-07: RED captured (E0308 `expected &StreamDecoder, found
      closure`); entry is `pub fn decode_span_synthesis(inputs,
      &dyn Fn(&[f32]) -> Vec<StreamUtterance>)`; contract test pins
      once-per-stream call order, identity skip, cluster/margin/rms
      propagation, joined text. BOTH production call sites (mass loop +
      repair windows) now route through it — the mass inline loop is gone,
      so production itself has one decode path (identity-degrade mass spans
      now log the diagnostic warn; wording generalized to "synthesis span").
- [x] 1.2 Wire the gate replay's mass and repair decode loops through the
      closure-parameterized `decode_span_synthesis`; assert parity by
      replaying the committed census shapes (token-only) against the
      current render.
      DONE 2026-10-07: both gate loops (mass + repair) now call
      `commands::decode_span_synthesis` with their own engine closures —
      the mirror loops are deleted, parity is structural (same entry, same
      StreamEvidence construction). Gate compiles clean. The census-shape
      replay clause is fulfilled by section 4's harness (committed
      token-only fixtures driving the same entry) — ordered before any
      cold gate, so the assertion exists before the next gate run.

## 2. Class E — rate floor + duration floor (design D2)

- [x] 2.1 RED: unit tests pinning the two floors — a span below
      `MIN_SYNTH_DURATION_SECS` (1.5) degrades to the byte-identical
      mixture render; a stream decode whose words/sec (denominator: the
      stream's own span duration, one definition everywhere) exceeds
      `MAX_STREAM_WORDS_PER_SEC` (8) degrades that stream to no text;
      census records the firing floor. Repo fixtures PURELY synthetic
      (invented shapes); the six real spans' shapes live in the evidence
      home.
      DONE 2026-10-07: `impossible_rate_stream_decode_degrades`
      (run_assembly: 15 words / 1.2s span → None; 3 words → Some) and
      `duration_floor_stands_down_before_separation` (run_engine:
      CountingSeparator proves the below-floor span never reaches the
      port). Both captured RED, then GREEN.
- [x] 2.2 GREEN: implement BOTH checkpoints where they belong — the
      duration floor as a cheap pre-separation filter on the span list
      (run_engine's span loop, before `port.separate` — never waste the
      GPU work); the words/sec ceiling post-decode in the synthesis
      acceptance (`vetted_stream_pair` path). Pinned by test that the
      duration floor fires BEFORE separation is invoked.
      DONE 2026-10-07: constants in run_assembly
      (MIN_SYNTH_DURATION_SECS=1.5, MAX_STREAM_WORDS_PER_SEC=8.0, dial
      docs); run_engine span loop skips + logs
      `CENSUS-STANDDOWN ... floor=MIN_SYNTH_DURATION_SECS` pre-separation;
      vetted_stream_pair logs `floor=MAX_STREAM_WORDS_PER_SEC` with the
      measured rate and rejects; synthesize_overlap_rows logs
      `CENSUS-SYNTH-DEGRADE` with the veto reason (mass path previously
      silent). Lib board 765 green.
      BUILD-PANEL AMENDMENTS (2026-10-07, adversarial review): (1) the
      duration floor is ALSO at the acceptance layer — both splices
      (synthesize_overlap_rows None, repair_overlap_rows_checked Err)
      reject sub-floor spans so the spec's unconditional "SHALL NOT
      synthesize" is structural for self-built SpanSynthesis (harness,
      future callers); two pre-existing test spans sat below 1.5s and were
      retargeted (disfluency test, straddling property) — the floor's
      first real catches. (2) Stood-down spans' attribution semantics
      pinned: `duration_floor_stands_down_before_separation` now proves
      the span's atoms get NO separated vote and NO covered mark, and the
      mixture vote survives merge (byte-identical render promise).
      (3) Census hole reconciled in the spec delta: duration-floor
      stand-downs appear as CENSUS-STANDDOWN records only (no vote/RMS
      near-miss data — the GPU work the floor saves); the dial is
      retuned by raising the floor and re-running. (4) Mining story
      corrected: a SEVENTH span [2537.00-2538.52] passed all pre-floor
      gates at 22.4 w/s (masked by the applied clip-04 repair window in
      the final render) — the rate backstop catches it, validating the
      two-floor design; clip07 (6.8 w/s) defeats the rate backstop alone.
      No extra pin for the seventh span: it sits inside clip-04's
      sanctioned wide repair window, where a mixture-only pin would
      contradict the ear-ruled fix path.
- [x] 2.3 Gate pin: the words-per-second assertion + the duration-floor
      census record. Create NEW ear-truth fixture entries for the six
      spans with a synthesis-stand-down pin kind (a span expected to
      render mixture rows only) — the repo fixture has no entries for
      them today except S16, and the manifest's `[ok]` marks are local
      evidence, not fixture data.
      DONE 2026-10-07: `synthesis_standdown` pin kind asserted post-replay
      against the merged render (no synth atom may overlap the pinned
      walls; waiver path shared with the entry loop); six fixture entries
      `classE_clip07..12` (walls 0.37-1.03s, token-only notes; physics
      ruling per proposal, NOT ear-attested); gate inits env_logger
      (default warn) so the lib CENSUS-STANDDOWN lines reach the recorded
      log. Repair-window interaction documented in the clip-09/11 notes:
      a wide-context repair absorbing a pinned span FAILS the pin → user
      amendment, never a silent skip. Gate fixture lint green.
- [x] 2.4 Smoke spec: extend `frontend/e2e/smoke/speaker-diarization.spec.ts`
      — mock fixture mirrors the backend persisting NO synth row for a
      below-floor span; assert the post-refetch render shows the mixture
      rows; refresh the stale comment claiming the backend persists the
      two-per-voice-row shape unconditionally.
      DONE 2026-10-07: 15.3f (mixture row survives refetch, exactly one
      badge row — no fabricated pair); 15.3e comment now says synthesis is
      conditional. Spec file 10/10 green (chromium/Windows).

## 3. Fidelity scans (design D3)

- [x] 3.1 RED: boundary-token scan — pure fn over render rows
      (provenance-scoped: synth-tail vs neighbour-head, ≥3 shared tokens,
      no 80% bar); repo fixture reproduces the leak shape with synthetic
      text. GREEN: scan lands; production census + gate both call it.
      DONE 2026-10-07: `scan_boundary_leaks(rows, min_chunk)` +
      `BOUNDARY_LEAK_MIN_CHUNK=3` in run_assembly (per-pair: synth side's
      tail vs neighbour head AND head vs neighbour tail, largest chunk
      wins; plain/plain pairs never flagged; findings carry ids/walls/
      chunk_len only — no text, keeping the census bright-line safe).
      RED captured (E0425), then GREEN; one test-text bug fixed
      (mis-engineered tail). Reuses alignment's `normalized_tokens`
      (made pub(crate)).
- [x] 3.2 RED: parent-link integrity scan — `synth_parent` must be an
      atom link or a designed split-piece link; synth-span walls must
      overlap parent walls. GREEN: scan lands AND the ..ddfdee provenance
      misalignment is REPAIRED (this change owns the data fix; re-point or
      null the link) and the designed split-piece links are documented as
      such so the scan does not flag them.
      DONE 2026-10-07: `scan_parent_links(rows, sources)` +
      `ParentLinkDefect::{Dangling, WallMismatch}` in run_assembly (valid
      shapes documented on the fn: atom donor link + split-piece
      self-link). DATA FIX applied to the live meeting DB: the two clip-12
      synth rows [4304.37-4304.91] re-pointed from ...86ddfdee (walls end
      4292.57, 11.8s away) to ...0d35cff0 (the only source overlapping the
      span, [4292.57-4317.73]); pre-change rows verified, update scoped to
      non-overlapping walls only. DURABILITY CAVEAT (build-panel finding):
      persist_regenerated_rendering re-derives rows on every Speakers run,
      so the re-point covers persisted reads until the next re-run — at
      which point the class-E duration floor stands the span down (0.54s <
      1.5s) and no synth rows re-emerge with any parent. The scan cannot
      distinguish the CORRECT parent among several overlapping candidates
      — the re-point is a scan-green choice; the next re-run closes it.
- [x] 3.3 Census recording: both scans' findings land in the production
      census and the gate log (token-only), with near-miss rates; note the
      census-vs-live yield mismatch (P5 data gap) as a measured open item.
      DONE 2026-10-07: production logs CENSUS-LEAK /
      CENSUS-LEAK-SUMMARY (synth_rows vs flagged) / CENSUS-PARENT from the
      final pre-persist render (commands.rs, sources wall-joined); the
      gate runs the SAME fns over the replayed render and eprintlns the
      same token-only lines. Census-vs-live yield mismatch: still a
      measured open item — the probe (3.4) counts the PERSISTED render;
      the gate counts the replay; the two are compared only at a cold
      gate run (5.5 records it).
- [x] 3.4 Calibration checkpoint: review the near-miss rates on the census
      corpus, set the boundary-token dial, and decide the pinned-window
      scope BEFORE any gate enforcement lands (task 5.3 depends on this).
      DONE 2026-10-07: `tests/fidelity_census_probe.rs` (env-gated,
      DB-only, drives the real scan fns, token-only histogram) measured
      the persisted render: 281 rows, 24 synth atoms, 43 synth-side
      adjacent pairs vs 244 plain pairs. Histogram: synth-side max
      shared-chunk = 1 (2 pairs — "yeah"-style echoes); plain-side one
      chunk=4 pair (plain/plain, duplicate-scan territory) and zero ≥3.
      scan_boundary_leaks(3) → 0 findings; scan_parent_links → 0
      findings over 28 links (the ..ddfdee fix cleared the last one).
      DECISIONS: dial stays at 3 (comfortable margin over the chunk=1-2
      background); 5.3 enforcement scope = the three new scans asserted
      on the PINNED windows (S16/S18 + the six stand-down spans), not
      meeting-wide auto-failure — a meeting-wide flag stays census-only
      per D3.

## 4. Census-replay harness (design D6) — before any cold gate

- [x] 4.1 Harness skeleton in `tests/`: PURELY synthetic repo fixtures
      (invented walls/word counts, engineered defect shapes) + loader for
      meeting-shaped fixtures from `MEETILY_LOCAL_EVIDENCE_DIR` (skipped
      with a loud note when absent); drives the REAL `apply_overlap_synthesis`
      / `apply_loop_repairs` / `decode_stream_utterances`; fake-decoder
      contract keyed via per-call closure scoping + call counter; README
      stating the two-tier bright line.
      DONE 2026-10-07: `tests/census_replay_harness.rs` + 5 synthetic
      fixtures in `tests/fixtures/census_replay/` (replace happy path with
      per-utterance walls, impossible-rate degrade, punctuation-only
      degrade, same-badge degrade, low-RMS degrade, proportional split
      with byte-exact pieces) + README with the two-tier bright line.
      Per-call closure scoping pins: decode-once per stream + decode
      inputs byte-identical to the pinned samples.
- [x] 4.2 Property tests over the splice: word conservation (absorbed
      multiset = replacement union, modulo attested echoes), wall
      monotonicity + span clamping, provenance containment (donor ids ∈
      source set), absorption fixpoint — over synthetic fixture texts;
      meeting-shaped texts only via the evidence-home loader.
      DONE 2026-10-07: `straddling_split_never_invents_or_reorders_piece_words`
      (256 cases: pieces are ordered subsequences of absorbed words, walls
      monotonic + clamped ±1ms f64 round-trip, provenance contained — the
      property CAUGHT the 1ms f64→ms truncation artifact and shrank it to
      a minimal case) and `repair_application_is_an_absorption_fixpoint`
      (256 cases: re-application rejected, render unchanged). Full
      multiset conservation intentionally NOT asserted — stream decodes
      are independent texts; the honest invariant is piece containment.
- [x] 4.3 Regression fixtures for the three burned designs of 2026-10-05
      (per-chunk decode contract break, proportional word scatter, p90
      floor in bleed) — each fails at the replay tier in milliseconds.
      DONE 2026-10-07: v1 → `replace_two_utterance_streams` (decode-once +
      full-samples contract; a re-slice fails the byte-identical assert);
      v2 → `split_straddling_row_proportional` (piece texts pinned
      byte-exact); v3 → `degrade_low_rms_stream` (pre-normalization RMS
      floor degrades the amplified-residue stream).
- [x] 4.4 Ladder documentation in this change's `runbook.md`: unit → warm
      replay → span probe → cold gate; no cold run until replay is green.
      DONE 2026-10-07: runbook.md with the four-rung ladder, the pre-triage
      protocol stub (8.1 extends it), the 3.4 calibration record, and the
      open census-vs-live yield item.

## Build-stage adversarial panel (2026-10-07, 3 reviewers) — folded

Verdicts: 0 blocking defects; findings folded as follows.
- Scans panel: boundary scan restricted to TOUCHING edges only (the two
  non-touching pairings could only false-positive; design D3 wording
  reconciled) + `BOUNDARY_LEAK_MAX_GAP_MS=2000` shared-edge guard (same
  guard family as the duplicate scan) + 2 new guard tests; census source
  fetch abstains LOUDLY on DB error (an empty wall list would have turned
  every link into a false Dangling); ..ddfdee durability caveat recorded
  in 3.2 (next re-run closes it via the duration floor). Recalibrated
  after the fix: still 0 findings, dial stays 3.
- Class-E panel: floors verified sound on every real path; the census
  hole (stood-down spans lack vote/RMS data) reconciled via the spec
  delta's DURATION-FLOOR CARVE-OUT; stood-down attribution semantics
  pinned in the run_engine test (no separated vote, no covered mark,
  mixture vote survives); acceptance-layer floor added to BOTH splices
  (spec's unconditional SHALL-NOT is now structural); rate-floor tests
  retargeted to 1.6s spans (above the duration floor); mining story
  corrected (a seventh 1.52s/22.4wps span exists, masked by the clip-04
  repair window — the backstops catch it; no pin: it sits inside the
  sanctioned wide repair window).
- Harness panel: the "word conservation" reviewer finding was traced to
  the split loop — mid-words between the two proportional cuts are
  dropped BY DESIGN (in-span words owned by the stream rows; pinned by
  `split_never_duplicates_a_word_between_head_and_tail`); the honest
  property set is per-piece + COMBINED-piece ordered containment (added:
  catches the cross-piece duplication bug class). Reason-blindness fixed:
  `synthesis_veto_reason` (pub, same checks/order as the splice) + a
  `veto` field on every fixture — a vetting reorder now fails the replay.
  Decode contract tightened to EXACTLY the expected call count (2 for
  decisive, 0 for undecided identity); new fixtures `degrade_undecided_
  identity` and `degrade_abstaining_stream`; loader panics carry path+tier.

## 5. Gate fidelity pins (design D8) — after 3.4 calibration

- [x] 5.1 RED: multiplicity/badge-aware needle matcher as a pure fn +
      fixture pin schema (ordered per-row pins: badge selector + ordered
      tokens); a leak fragment under an unattested badge fails; an extra
      occurrence fails. GREEN: matcher lands.
      DONE 2026-10-07: `RowPin` schema (badge selector, ordered tokens,
      exact count, row|joined scope, hard flag) on Entry.pins + pure
      `count_token_run` (non-overlapping contiguous) + `check_row_pins`
      in the gate; 4 matcher unit tests (extra occurrence, leak doubles
      multiplicity under the badge-agnostic pin while the owner-badge pin
      stays green, chop-point straddle only in joined scope, count-0
      absence + non-overlapping counting). Semantics note: a leak under
      an unattested badge fails the MULTIPLICITY pin (badge-agnostic);
      the owner-badge pin pins the owner's copy.
- [x] 5.2 Migrate the hand-coded S16/S18 assertion blocks onto the pin
      schema. S16 pin shape decided per design (post-M1: mixture-only
      rows, needles carried by the plain row at attested multiplicity);
      S18's joined-text semantics preserved via a window-level pin kind
      (needle straddling a row boundary still counts — chop points are
      not pinned). Fixture entries land FIRST (RED where fiction exists),
      then the gate flips to the matcher.
      DONE 2026-10-07: both hand-coded blocks deleted; S16 → joined-scope
      waivable pins (["or","idp","rather","than","search"] x1,
      ["what","skins"] x2 — attested multiplicity); S18 → joined-scope
      HARD pins (fabrication absence x0 + six attested needles x1, token
      forms post-`norm_tokens` — "it's" → ["it","s"]); generic pin
      enforcement runs over the FINAL replayed render (hard → render
      failure, waivable → the entry's amendment record). `row_pins`
      entry kind added (pin-only, no label-structure check).
- [x] 5.3 Boundary-leak + parent-link + rate gate assertions on pinned
      windows per the 3.4 calibration; the same-donor exemption narrowed
      to NOT suppress the three new scans (fracture/duplicate exemption
      semantics unchanged).
      DONE 2026-10-07: window-scan block after pin enforcement — for every
      pinned window (pins ∪ synthesis_standdown): boundary-leak findings,
      parent-link findings, and synth rows above MAX_STREAM_WORDS_PER_SEC
      are render failures. Scope = pinned windows only (3.4 decision);
      meeting-wide stays census-only. The fracture/duplicate scans' own
      same-donor exemption untouched (the new scans are provenance-
      scoped separately and never consult it).
- [x] 5.4 Class-B interim pin: the clip-04 dictated exchange lands as an
      ordered per-row pin + a KNOWN-LIMITATION amendment (the render
      flattens it until token walls land — the pin documents the defect
      mechanically instead of hiding it).
      DONE 2026-10-07: `clip04_mixed_row_render_2535` (row_pins kind) —
      HARD joined-scope pins guard the mixture text the repair destroyed:
      the GENUINE doubled "he was hired as a data analyst" x2 (user-
      cleared), the surviving "yeah yeah yeah" interjection x1 (the
      word loss the repair created), the roadmap answer x1. PRE-FLOOR
      PREDICTION (8.1 protocol, recorded before any run): the stutter
      seed dies with the stood-down sub-second span and the mixed row has
      no 5-word consecutive repeat → the repair window [2535.17-2559.11]
      no longer builds → the mixture row renders and the pins PASS; a
      repair re-fire fails them HARD. KNOWN-LIMITATION amendment records
      the user's net-worse ruling + the class-B flattening. Fixture now
      28 entries, 3 pinned; lint green; live gate compiles.
- [x] 5.5 Cold gate run via this change's own
      `tools/run_ear_gate.bat` runner (recorded to this change's
      `gate-runs/`): REQUIRES harness green (section 4) and 8.2 done. The
      recorded log must show: the six fiction spans stood down (rate/
      duration floors), all existing PASS/AMENDED entries unchanged, new
      fidelity pins green.
      DONE — RUN 4 (20261008-020626.log, ~29 min): FIRST FULLY GREEN
      COLD GATE. 27 passed, 1 known-limitation (S16), 0 FAILED of 28
      entries; exit 0. The clip-09 wide window [1421.21-1453.63]
      degraded on the landed dial ("a stream utterance exceeds the
      words-per-second ceiling against its own walls" — the phantom's
      10 w/s signature); the S18 window applied (1/3, the 2821 collapse
      unchanged); duplicate scan 0 pairs; clip-04 pins PASS.
      RUN 1 RECORDED (20261007-cold-gate-m1.log, 31 min): the floors
      worked (130 duration stand-downs pre-separation; candidates 132→2;
      all six classE pins PASS; S16 amended, S18 pins PASS; token census
      CENSUS-TOKENWALL x8 recorded for 6.3; window scans 0 leak/parent/
      rate findings). THREE failures, all real catches:
      (a) clip04 hard pins x2 — the [2535.17-2559.11] repair re-fired
      from a stutter seed whose text was rate-floor FICTION (the
      [2537.00-2538.52] candidate, 34 words/1.52s). FIXED
      post-run: `is_class_e_fiction` — fiction candidates seed no repair
      (both seed sites filtered; general dial implementing the user's
      2026-10-06 net-worse ruling; `class_e_fiction_candidates_seed_
      nothing` pins it).
      (b) classE_clip09 — edge absorption + a NEW loop seed (the fiction
      rows now render as plain mixture rows and one trips the 5-word
      repeat) built a wider window [1421.21-1453.63] whose per-voice
      render covers the pinned sub-second span. This is the PRE-REGISTERED
      user-amendment case (fixture note): wide-context repair re-synthesis
      over a stood-down span is ruled by ear, not physics. BLOCKED ON
      USER RULING — no self-amendment.
      (c) duplicate_cluster [1421.21]+[1429.01] — a NEW double-decode
      pair inside that same window (no waiver class). Needs row-level
      diagnosis (the [1421] window's synth-row/consolidation geometry)
      at the next gate iteration.
      A re-run after (a)'s fix + the user's clip-09 ruling is the
      remaining work.
      RULING RECORDED (2026-10-07, clip-09 probe transcript + audio):
      the wide render is WRONG — the user attested the stream-0 row
      [1429.0-1429.2] "You don't?" is a PHANTOM duplicate of Speaker 2's
      own words ("you don't control what categories...") falsely
      attributed to the other speaker; Speaker 0's [1446.3-1451.1] row is
      genuinely the other participant but its "Thank you." tail was NOT
      heard; and the badge fact Speaker 2 = the meeting owner (the
      user's own voice) — earlier name glosses were false. This also
      diagnoses (c): the duplicate_cluster pair is
      the phantom row. Mixture render is the ruled preferred state.
      DIAL LANDED (RED→GREEN, 3 new tests; clip-09 shape reproduces the
      exact RUN-1 span (1421210,1453630) as the RED failure):
      `repair_windows_from_seeds` takes the duration-floor stand-down set
      (`stood_down_ms`, from `duration_floor_standdowns(engine overlap
      spans)`, threaded through commands.rs's 8-tuple and the gate
      replay); a window covering a stood-down span stands down ENTIRE
      (CENSUS-STANDDOWN ... window covers stood-down span) — truncating
      would still render the covered head's phantom. Touching windows
      (clip-11 geometry) still fire. Fixture: classE_clip09 amendment
      user_confirmed 2026-10-07 + note updated; probe badge gloss
      scrubbed to cluster indices.
      RUN 2 RECORDED (20261008-002305.log, ~16 min): the geometry dial
      cleared clip-04 (fiction-no-seed), classE_clip09 (window blocked,
      pin PASS) and the duplicate scan (0 pairs — the phantom gone), BUT
      it FLIPPED S18: its repair window [161.71-191.05] also covers a
      floor span [173.02-173.71], the dial blocked it, and the mixture
      render brought the fabrication back ("i think it s a good idea"
      x3, absence pin got x3). VERDICT: geometry cannot express both ear
      rulings — the same property (covering a floor span) is attested
      GOOD in S18 and ruled BAD in clip-09. REVERTED the geometry dial
      (param, threading, 8-tuple, 3 tests); replaced with the
      quality-scoped SEPARATION-LEAK VETO the phantom's own signature
      points at: `vetted_stream_pair` degrades any synthesis whose two
      stream decodes share a verbatim >=3-token normalized run
      (`shares_long_token_run`, bar = BOUNDARY_LEAK_MIN_CHUNK; calibration
      3.4 measured the synth-side background at shared-chunk <= 1). The
      clip-09 window then degrades at acceptance (phantom + unheard tail
      never render), S18's clean decodes are untouched, and any future
      leaky window over a floor span still fails the classE stand-down
      pins at the gate. RED→GREEN: `cross_stream_duplicate_run_degrades_
      the_synthesis` (phantom shape; both splices) + `two_token_echo_
      across_streams_still_synthesizes` (real talk-over echo survives).
      RUNNER FIX: run_ear_gate.bat was LF-only with non-ASCII dashes —
      cmd executed fragments of its own comment lines ('ation-render-
      fidelity' is not recognized). Rewritten ASCII-only + CRLF; guards
      probe correctly. RUN 2 is recorded through the runner's exact cargo
      command line (bash transport: the bat's PowerShell/cmd wrappers
      don't survive MSYS background invocation on this machine path).
      RUN 3 RECORDED (20261008-010407.log, ~16 min): clip-04 PASS,
      classE_clip09 PASS, duplicate scan 0 pairs — BUT the 3-gram veto
      ALSO degraded the S18 window [161.71-191.05] ("a stream decode
      duplicates the other stream's words"), the mixture render brought
      the fabrication back, and S18's absence pin failed post-repair.
      SPAN PROBE (s18_leak_probe.rs, verbatim terminal-only): the two S18
      stream decodes share "would have been" — two REAL speakers using
      the same ordinary phrase in one 29s window; ANY n-gram bar over
      whole-window texts over-fires. The probe also exposed the phantom's
      true mechanical signature: "You don't?" rendered as its own
      utterance at 2 words / 0.2s = 10 w/s — over the EXISTING class-E
      ceiling, at the utterance granularity instead of the span.
      DIAL v3 LANDED (the 3-gram veto removed): vetted_stream_pair now
      applies MAX_STREAM_WORDS_PER_SEC per UTTERANCE against its own
      utterance walls (CENSUS-STANDDOWN ... utterance=[..] rate=..w/s).
      RED→GREEN: `fiction_utterance_rate_degrades_the_synthesis` (the
      phantom shape, both splices) + `human_rate_utterances_still_
      synthesize` (S18's phrase echo + long utterances survive). A false
      veto degrades to the mixture render — text-safe by construction
      (attested pins read the joined render), costing only badge
      granularity. RUN 4 pending: prediction — 1421 degrades (phantom
      rate), 161 applies (long utterances), 2821 collapses as before →
      28/28.

## 6. Token census — phase A only (design D5)

- [x] 6.1 RED: a structural params test pinning that
      `transcribe_span_blocking` keeps `no_timestamps(true)` (byte-stable
      decode); GREEN: the sibling adapter method
      (`transcribe_span_blocking_tokens`) flips timestamps locally,
      preserving spawn_blocking semantics. NO equality test between the
      two decodes' text — the token decode's chunking differs by design.
      DONE 2026-10-07: `params_structure_tests::transcribe_span_blocking_
      keeps_no_timestamps_true` (source-structure pin: the strict profile
      holds no_timestamps(true), the production fn routes through it and
      never touches the posture, the sibling flips it locally; RED at the
      sibling-expect, GREEN after). `transcribe_span_blocking_tokens` +
      `SpanTokenSegment` added (blocking_read like the production decode,
      spawn_blocking-only; walls span-relative ms, zeros when a segment
      carries no usable tokens). No equality test exists.
- [x] 6.2 GREEN: census records token walls + the token decode's text
      sha256 alongside the production decode's as a divergence NOTE (never
      an equality assertion); NO shape change, NO wall consumption — data
      only.
      DONE 2026-10-07: the gate's repair replay emits CENSUS-TOKENWALL
      lines per repair stream (window walls, segment count, per-segment
      token walls, token-decode sha + production-decode sha). Production
      GPU cost unchanged — the token decode runs ONLY in the gate replay
      (the evidence run); the production census stays scan-only.
      SpanSynthesisInput gained #[derive(Clone)] for the census pass.
- [x] 6.3 Wall-reliability evidence plan: record token walls for the six
      long repair windows across a cold gate; the ear round (task 4.4 of
      the previous change, moved here) validates reliability before any
      phase-B shape work.
      DONE 2026-10-07: plan recorded in this change's runbook.md — the
      5.5 cold gate records CENSUS-TOKENWALL for every long repair
      window; the ear round replays clips against those token-wall
      boundaries; only ear-confirmed walls graduate into the splice
      (phase B, a future change).

## 7. Edge absorption (design D7)

- [x] 7.1 RED: `repair_windows_from_seeds` absorbs a straddling edge row
      WHOLE within `REPAIR_CHAIN_CAP_SECS` (35 initial; new constant,
      distinct from `OVERLAP_MAX_SPAN_SECS`); cap-exceeding chains stop
      before the edge row; partial coverage never introduced. Fixture: the
      clip-03 edge shape (synthetic).
      DONE 2026-10-07: `repair_chain_absorbs_straddling_edge_row_whole`
      (RED: old chain trimmed at the mass cap) and
      `repair_chain_stops_before_cap_exceeding_edge` (absorb-whole-or-
      stop), both over the synthetic clip-03 shape.
- [x] 7.2 GREEN: constant + absorb-whole-or-stop logic; mass-span guard
      unchanged (pinned by test).
      DONE 2026-10-07: `REPAIR_CHAIN_CAP_SECS = 35.0` (own constant, doc
      states the never-weaken-mass-eligibility rule); both chain loops
      absorb a straddling edge row (start inside the cap window, extended
      total within the chain cap) then STOP; repair_overlap_rows_checked's
      own cap moved to the chain cap (a 31s absorbed window synthesizes,
      36s errs — `repair_splice_accepts_windows_up_to_the_chain_cap`);
      the MASS splice keeps OVERLAP_MAX_SPAN_SECS
      (`oversized_span_degrades`). Three pre-existing chain tests pinned
      the OLD stop-at-mass-cap behavior and were updated to the D7
      semantics (the absorbed rows verifiably straddle: start < cap line,
      extended ≤ 35s). Board 775 green.
- [x] 7.3 Post-land check: re-run the boundary-leak scan on the repaired
      windows — the edge absorption must not introduce new leak pairs
      (sequence per design: after 3.1).
      DONE 2026-10-07 (two halves): the persisted-render half re-ran via
      the calibration probe — 0 boundary-leak findings (the persisted
      render predates the absorption; nothing changed under it). The
      repaired-window half happens at the 5.5 cold gate: the replay builds
      the windows with the NEW chain logic and the gate's window-scan block
      (5.3) asserts no boundary-leak findings on every pinned window —
      recorded in the same run.

## 8. Ear protocol + close-out

- [x] 8.1 Pre-triage runbook (this change's `runbook.md`, started at 4.4):
      agent predicts the class per clip from the census profile (duty
      cycle, chunk count, margins) before handing clips to the user;
      class-stratified clip sets.
      DONE 2026-10-07: runbook pre-triage section extended with the worked
      example — the clip-04 prediction (repair seed dies with the stood-
      down span; mixture row renders) recorded BEFORE any run, encoded as
      the entry's hard pins, adjudicated by the cold gate before ear time
      is spent. Clips 07-12 dropped from ear rounds entirely (physics
      ruling, proposal).
- [x] 8.2 S16 KNOWN-LIMITATION wording update in the fixture (BEFORE the
      5.5 cold gate): with the fiction rows stood down the window renders
      single-badged mixture rows — the documented accepted limitation; the
      amendment record must describe that truth.
      DONE 2026-10-07: S16 amendment wording updated (post-floor truth:
      single-badged mixture rows whole, needles on the plain row at
      attested multiplicity, attribution/ordering still the accepted
      limitation); done together with the 5.2 pin migration since the
      amendment and the pins describe the same state.
- [ ] 8.3 Full verification: `cargo test` (lib + replay harness + lint),
      smoke specs, one cold gate — all green; tasks closed; deltas re-read
      against the implementation before any archive move.
