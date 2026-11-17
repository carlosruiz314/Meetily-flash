## 1. Offline guard validation — settle the bar before any production threshold

- [x] 1.1 RED in `frontend/src-tauri/tests/net_worse_guard.rs` (plain
      `cargo test`, invented generic tokens in the existing
      `run_assembly.rs` test style — never captured real sequences): the
      per-badge ordered-alignment statistic — fluent substitution collapses
      below any sane bar; word-order scramble degrades it (multiset
      equality alone does NOT pass); the interleaved two-badge baseline vs
      stream-serialization fixture scores ≈1.0 per badge where a GLOBAL
      alignment would score ≈0.5 (global alignment is rejected by test);
      within-badge re-partitioning costs at most a small pinned penalty;
      recall denominator excludes unscopable repair-side tokens;
      normalization scope (case, punctuation, whitespace) pinned.
      [DONE 2026-11-17: 10 red-first tests in `tests/net_worse_guard.rs` —
      perfect conservation, substitution collapse ≤0.5, multiset-equal
      scramble ≤0.4, interleaved fixture per-badge ≥0.95 vs global
      11/20≈0.55, boundary-blind repartitioning, unscoped-token
      recall/precision, normalization, badge flip ≤0.3, missing-stream
      hard miss, empty-baseline skip]
- [x] 1.2 Implement the statistic as a pure function in the speaker
      run-assembly layer; green. [DONE 2026-11-17:
      `src/audio/speaker/net_worse_guard.rs` — `normalize_tokens`,
      `lcs_length`, `ConservationGroup`/`RepairStream`/`GroupAlignment`,
      `align_groups` (missing badge = hard 0.0 miss, empty baseline emits
      no outcome); registered in `speaker/mod.rs`. 10/10 green.]
- [x] 1.3 Env-gated live probe (`#[ignore]`, `MEETIFY_LIVE_DIAG`,
      inheriting all `span_decode_probe.rs` conventions: early-return
      without the env, verbatim only under `MEETIFY_RENDER_PRINT`, sha256 +
      counts otherwise; clips/WAVs written ONLY under the evidence home or
      the meeting folder): for a named window shape + seed span, run
      separation + both stream decodes, assemble the repair render via the
      existing builder, compute the per-badge scoped ratios against the
      window's mixture rows.
- [x] 1.4 Freeze the statistic FIRST (denominator, normalization,
      minimum-set handling — recorded in the change record before any
      number exists), then run the probe on the three kept window shapes
      (validated 32.69s, mangled 30.68s, 12s control) and record: per-badge
      ratios (recall + precision), divergence localization, the word-loss
      disposition as the ENUM `seed-span-interior | conservation-set`
      (never text), raw ratios + full alignments in the evidence home, and
      the reproduction criterion — the re-derived validated render passes
      that window's fixture pins and the mangled-case render fails the same
      pins; a mismatch is a recorded STOP for the user to rule on.
- [x] 1.5 STOP-POINT: (a) hard non-seed-interior conservation holds → hard
      interior invariant replaces the ratio; (b) hard conservation FAILS →
      the bar is USER-RULED as a dated class dial — mandatory, with a
      mechanically pinned separation margin between validated and mangled
      ratios (an implementer-chosen midpoint does not ship), the 12s
      control's expected verdict pinned in the same ruling, and the bar
      bound to ear-gate calibration governance with hold-out entries
      excluded (verify the attested windows map to non-hold-out entries).
      Record the dated constant + evidence sha256s in the change record.
      No production wiring proceeds before this settles. [ESCALATED 2026-11-17: the measured evidence shows NO conservation bar separates the validated render (0.652/0.857) from the mangled one (0.818/0.903) — the mangled case scores HIGHER; the guard premise is falsified for short fluent substitutions. Disposition pending the user's ruling; production wiring does not proceed. USER RULING 2026-11-17: drop the repair for good, keep the waiver-scoping fix; the measured verdict closes issue #6.]

## 2. Net-worse guard — the production decision function

- [x] 2.1 RED in `net_worse_guard.rs` (plain `cargo test`, invented
      tokens): guard verdict over (baseline mixture rows, repair render
      rows, seed walls) → apply vs stand-down per the settled bar;
      fail-closed cases (empty baseline, missing rows, guard error →
      stand-down); EMPTY conservation set → no promotion; TWO seeds in one
      window → one union window, one verdict, deterministic order; badge
      flip on conserved text → both groups degrade; manual-row-in-window →
      no promotion; floors-synthesized-overlap → no promotion; census
      record shape token-only.
- [x] 2.2 Implement the guard in the run-assembly layer (pure decision, no
      I/O, no fixtures — design D1/D9), factoring the fused build+splice in
      the repair fold into decide-then-splice; green.
- [x] 2.3 Window-edge exemption band ONLY if task 1.4/1.5 evidence pinned
      one: red test + implementation + the band value recorded with its
      evidence source (design D4). If no band is pinned, record that
      explicitly and skip.

## 3. Rebuild the scramble-seed promotion behind the guard (from scratch)

- [x] 3.1 Rebuild the seeding/evidence pass from fork issue #6 + the
      archived `2026-10-08-overlap-stream-retranscription` record (the
      dropped branch left NO commits — verified 2026-11-17; nothing to
      recover). Detection as pure functions + census probes ONLY — NO
      production wiring in this task; red test asserts no promotion is
      reachable from the repair fold yet.
- [x] 3.2 RED (plain `cargo test`, recorded evidence arrays): seed
      definition — trigger-fired, REACHED separation (duration-floor
      carve-out excluded), final outcome the FULL mixture render (partial
      synthesis excluded), separated decode exceeding the pinned ceiling;
      the fingerprint decode's text is never rendered; the containing
      window builds by the existing repair-window rules; two seeds → one
      union window; window priority stutter → loop → scramble (dedupe-to-
      first order pinned); `LoopRepairCandidate` carries a seed-kind
      discriminator so ONLY promoted windows route through the guard.
- [x] 3.3 Wire promotion through the guard (green): enters as the third
      seed source in the post-`resolve_duplicate_clusters` repair fold;
      violation, error, or missing baseline → byte-identical mixture
      render; manual-row and floors-region blocks enforced; promoted rows
      carry `original_id`/`synth_parent`, the continuation fact, and
      synthesis-atom status, and pass the hallucination guards + ceiling at
      rendered granularity; the production census records EVERY seed fire's
      outcome (applied / guard stand-down / window not built / separation
      unavailable) token-only.
- [x] 3.4 Resumable census cache (design D8): keyed by window walls +
      absorbed-row id set + model versions (NOT span walls alone);
      atomic writes; RED cases — cached span skipped, key change /
      incomplete / corrupt entry re-decodes, resolved cache root verified
      outside `git rev-parse --show-toplevel`, cache schema holds decode
      evidence only (never verdicts, never a text field). Plain
      `cargo test`.

## 4. Ear gate — pin-scoped waivers (no more silent absorption)

- [x] 4.1 RED (plain `cargo test`): extract waiver resolution into ONE pure
      router and red-test ALL FOUR call sites (kind-keyed
      `record_render_failure` ~:422, entry-verdict waivers ~:818 and
      ~:1479, pin waiver ~:1978): a failing waivable pin whose label no
      complete amendment record covers FAILS even though the entry is
      listed in `known_limitations`; a covered pin resolves
      KNOWN-LIMITATION; verdict-level failures resolve via enumerated
      failure-kind covers; unlabeled waivable pins are never coverable; no
      wildcard; hard pins are never waivable regardless of covers.
- [x] 4.2 Implement: `RowPin.label`, `Amendment.covers` (pin labels ∪
      enumerated failure-kind ids), router wired into all four sites;
      `ear_truth_fixture_lint` extended to BOTH gate fixtures (date +
      reason + non-empty covers whose entries resolve — pin labels present
      on the entry, or known failure-kind ids); green.
- [x] 4.3 Local evidence-home fixture migration (edits the evidence home,
      NOT the repo): label the limited entries' waivable pins, populate
      each existing amendment record's covers from its documented reason —
      including check-kind covers for the two entries whose limitations
      waive entry-verdict failures (their entries carry only hard pins, so
      pin-label covers alone would be unsatisfiable); re-run the lint
      green; record the edit in the change record at record-count altitude
      only ("N amendments gained covers") — never transcribing record
      bodies.

## 5. Verification + closure

> Tasks 2.1–2.3, 3.1–3.4, 5.2 and 5.3 are CHECKED ABOVE as closed-by-ruling
> (2026-11-17): they are not executed — the repair path they build was
> dropped on the measured evidence with the user's explicit ruling. Their
> descriptions remain for the evidence trail.

- [ ] 5.1 Full `cargo test` green including the fixture lint (no
      concurrent cargo during live runs).
- [x] 5.2 Census-only pass FIRST (background, heartbeat): run the seed
      census and record the seed-fire census (how many seeds fire, where)
      — REVIEW the recorded census before any render changes; correct the
      seed-decode-set exposure claim if the census disagrees with it.
- [x] 5.3 Production Speakers re-run (heartbeat); verify mechanically by
      extending the existing replay/oracle harnesses
      (`census_replay_harness.rs` / `persistence_oracle.rs`) with named
      assertions: the ~192s seed's window is BLOCKED (floors-region
      overlap) and/or stands down while the region's floors render passes
      its pins; the validated window's render passes its pins;
      `transcript_sources` byte-identity; every seed fire's outcome present
      in the production census.
- [ ] 5.4 Cut clips ONLY for attestation-unpinned promoted regions (written
      ONLY under the evidence home or the meeting folder); if none exist,
      record that the ear round closed mechanically. Ear round on those
      clips only; dated fixture amendment only for user-ruled outcomes.
- [ ] 5.5 Smoke-spec deliverable decision (made at proposal time per
      AGENTS.md §3): the promotion changes WHICH rows the engine derives,
      not the render surface — the event→refetch→render wiring for
      coarse-rows-replaced-by-N-distinct-badge-rows is already pinned by
      `frontend/e2e/smoke/diarization-speaker-split-persistence.spec.ts`
      (and `diarization-label-quality.spec.ts` at N=2), and the engine-side
      selection is verified by the cargo gates + the ear-truth fixture
      pins. No new smoke spec is landed; this task records the rationale
      and verifies the covering specs still pass. [VERIFIED 2026-11-17: `diarization-speaker-split-persistence.spec.ts` + `diarization-label-quality.spec.ts` — 2 passed (1.4m); the re-scoped change touches no render-path code.]
- [ ] 5.6 Before archive: re-read the live spec + design against what
      landed; amend the delta/design first if the implementation evolved. [DONE 2026-11-17: artifacts re-scoped to the ruling before any archive — proposal/design/delta/tasks amended, validated 4/4; the landed code (statistic + scoping + probe + waiver router + lint) matches the amended design Outcome/D7/D9.]
- [ ] 5.7 `/opsx:archive` on the user's go; close fork issue #6 with the
      outcome comment (commits and pushes only on the user's explicit
      word).
