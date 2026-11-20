## 1. Candidate census — measure the flagging before touching anything

- [x] 1.1 Extend `tests/overlap_census_probe.rs` (DRY — no parallel probe;
      env-gated, `MEETIFY_LIVE_DIAG`, loud evidence-home skip, token-only):
      enumerate every overlap-span candidate flag from
      `engine_out.overlap_spans` (computed independent of enrollment, so
      un-enrolled meetings measure too) with wall position, overlap mass,
      duration, floors verdict (fires / stood-down), sorted by position;
      counts + sha256 only. [DONE 2026-11-18: probe extended (production
      pre-normalization RMS ratio semantics, wall-level floors verdicts,
      position-sorted record pinned by sha256) and run live on the reference
      meeting — record sha256 ebcd986c958382b2066e84495dabdd7e4671f8cfc5e7
      ad06b166e9a2b87b9f2e; note written to `census-review-note.md`.]
- [x] 1.2 Review the recorded census with the user (STOP gate); the review
      produces a dated token-only note in the change folder (counts +
      ruling) — a hard prerequisite for opening §3. [DONE 2026-11-18:
      the user's review ran through the census findings to the decode-
      reliability ruling ("decode reliability first, then ear pins",
      2026-11-18) and the three follow-up rulings on the probe outcomes
      (re-measure row-aligned / investigate the mega-row / token-walls
      change first); census-review-note.md review-outcome section
      amended with the dated ruling — no promotion wiring before it.]
- [x] COMMIT CHECKPOINT: commit the census record + note before §2 (the
      dropped predecessor lost everything; §7's rule). [DONE 2026-11-18:
      83548572.]

## 2. Decode reliability — offline, red-first, measured lever ladder

- [x] 2.0 RED (structural): the retry decode profile is unreachable from
      the general chunk path AND the mass overlap pre-pass (no call edge;
      the pre-pass keeps the plain profile); the retry variant uses a
      CLONED decoder context (`set_no_context(true)` on the clone), never
      mutation of shared params.
- [x] 2.1 RED (plain `cargo test`, invented generic tokens): the FROZEN
      selection rule — pass 1 stands unless it tripped rate-fiction or a
      stutter/hallucination guard AND the alternate pass clears every
      rejection guard; at most one alternate; both passes' evidence in the
      decode record (walls, per-pass token counts, guard trips, retry
      count, decode sha256).
- [x] 2.2 RED: malformed/empty decode handling — a rebuilt stream decoding
      empty must not replace non-empty mixture rows (stand down, record).
- [x] 2.3 RED: union-window rules — two seeds union deterministically; the
      UNIONED interval re-runs manual-row and synth-atom checks (RED test
      for the straddling-manual-row union); refused union falls back to
      individual windows if individually clean, else `window-not-built`.
- [x] 2.4 Offline lever-ladder probe on the three kept window shapes
      (env-gated, evidence-home fixtures, token-only record): the probe
      drives the FULL builder+splice path; leg A = live rebuild width +
      existing strict profile; leg B = leg A + the named no-condition
      retry; comparison operator FROZEN before measurement (amended at
      the tick to the gate's own pin semantics — the fixture pins are
      needles, not full sequences; design D3 retracts the exact-sequence
      wording). [DONE 2026-11-18: probe written + run live; record
      sha256 31e09fa22bcd3d933dde11996c84cb42bb2092b55a9b968980fd555335b
      290f7; clean shape RECOVERED 7/7 hard pins on leg A — leg B
      identical everywhere, no retry ships; validated shape NOT
      recovered (pin1 want=2 got=1) — diagnosed as wall-coverage, not
      decode: needle occurrence #2 sits beyond the recorded walls on
      today's geometry and the absorb-whole splice erased it (17.12s
      east); baseline holds 2/2, so the runtime stale-attestation hash
      gate fails exactly this closed — D4 amended: entries are cut
      ROW-ALIGNED on the hash-pinned render; control fail-closed at the
      vote bar (margin 0.046 < 0.05). STOP recorded: S16 entry awaits
      re-cutting row-aligned on the current render + re-measurement
      before §3 wires (probe-record-2-4.md).]
- [x] COMMIT CHECKPOINT: commit the probe record + chosen leg before §3.
      [DONE 2026-11-18/19: 0846f2c1 + addendum 8666a906 + investigation
      aef945fb; chosen leg A (no retry ships, leg B identical everywhere),
      STOP on S16 recorded; pause commit 334f951a.]

## 3. Promotion wiring — rebuild from the archived sketches

> **PAUSED 2026-11-18 (user ruling); RESUMED 2026-11-20 (user's go).**
> The pause's gate is cleared: `render-token-walls` landed 2026-11-19
> (15/15, reviewed to convergence, archived 2026-11-19-render-token-
> walls; delta synced to the main spec). The render is wall-bearing
> (census rows=269 walled=194 null=75 violations=0, two live runs
> byte-identical). The S16 re-cut is measured on the wall-bearing render
> — see `handoff-note.md` in the archived change: [1024.75,1056.50]
> (~31.75s ≤ cap), west edge row-aligned with the NULL-walled row's end
> (no absorb, no erase), east edge token-aligned at 1056.50 on the
> walled mega-row (occurrence #2's tokens survive in the east piece);
> baseline pins hold (S16 2/2, S18 7/7), builder outcomes unchanged,
> margins identical to 2026-11-18. §3 wires on that geometry.

- [x] 3.1 RED: two seed routes — (a) a duration-passing span whose
      separated decode trips rate-fiction or stutter; (b) an attested-window
      seed: a trigger-fired span whose walls fall within an allowlist entry,
      rebuilding REGARDLESS of the duration floor (RED test: a 0.4s
      attested-window span rebuilds; the same walls without an entry do
      not). `LoopRepairCandidate` gains the seed-class discriminator;
      negated-reachability test first (no promotion reachable from the
      repair fold until wired); RED test that an overlapping, token-similar,
      different-badge span pair SURVIVES `resolve_duplicate_clusters` (pins
      the existing overlapping-pair guard against regression); the
      fiction-candidate seed filter is explicitly amended for the
      decode-tripped talk-over class — design D4's supersession of the
      2026-10-07 ruling recorded. [DONE 2026-11-20: RED confirmed as
      compile-fail on the new API, then green — `RepairSeedClass`
      {Legacy, TalkoverDecodeTripped, TalkoverAttested} on
      `LoopRepairCandidate` (legacy builder stamps Legacy; talkover builder
      re-tags by seed class); `talkover_decode_tripped_seeds` (rate-fiction
      OR stutter on any stream — the D4 fiction-filter amendment for this
      class only, why-comment at the fn; legacy stutter route's exclusion
      untouched, its existing test stays green); `AttestedWindowEntry`
      (six fields, `deny_unknown_fields` already — hardening tests land in
      3.3) + `talkover_attested_seeds` (meeting-scoped, trigger span
      CONTAINED in entry walls seeds the entry's exact walls; straddling
      is not coverage); tests: attested sub-floor rebuild + four
      fail-closed negatives, fiction/stutter/clean seed triple,
      seed-class triple incl. negated reachability (legacy fold never
      talkover-classed), overlapping different-badge pair survives
      duplicate resolution (pinned existing guard — green immediately).
      Lib 810 green at -j 2.]
- [x] 3.2 GREEN: route the promoted window through the live repair-window
      builder (separation + per-stream decode + per-voice splice with
      `synth_parent` links); all existing rejection guards apply unchanged
      (margin-gated distinct-voice votes, hallucination guards, RMS
      eligibility, duration floor, w/s ceilings, manual-row-wins,
      byte-identical degrade, `transcript_sources` byte-identity). [DONE
      2026-11-20: the fold routes talk-over candidates through the SAME
      re-synthesis batch as the legacy windows (one separation + decode
      pass; `overlap_spans` threaded out of the engine blocking task as
      the 8th tuple element) — `talkover_repair_candidates` composes both
      seed routes into the class-aware builder (all guards live inside it,
      unchanged); apply order attested → legacy (seed priority) →
      decode-tripped (`order_repairs_for_apply` returns classes in
      lockstep); the splice-side gate filters talk-over windows to exact
      entry matches (`attested_entry_for_span` — the ms conversion is ONE
      shared fn with the seed route, the S16 re-cut walls round-trip
      pinned). INTERIM FAIL-CLOSED STATE, noted in the fold: the allowlist
      store read lands in 3.3 — until then entries are empty, so talk-over
      windows build + collect decode evidence but NOTHING splices (the
      render stays byte-identical at this commit). Tests: composition
      (both routes seed; decode-tripped overlapping attested dedupes into
      it; manual-row-wins), exact-match gate quadruple + fractional-walls
      round-trip, apply ordering + class lockstep. Lib 813 green at -j 2.]
- [x] 3.3 RED→GREEN: the attestation gate — `attestedRepairWindows` in the
      Tauri store (`settings.json` scheduler-settings mechanism): entries
      exactly `{meeting_id, start_s, end_s, attested_date, render_sha256,
      profile_fp}`; `deny_unknown_fields` (unknown field ⇒ treated as
      empty + loud warning); malformed setting ⇒ empty for that run;
      same-meeting exact-wall equality; hash + fingerprint must match
      (mismatch ⇒ `stale-attestation`, both hashes logged); unpinned ⇒
      mixture render byte-identical + `awaiting-attestation`; production
      never reads the evidence fixture (structural test); phrase-loop
      windows splice WITHOUT an allowlist entry (RED test pins the
      class-scoping). [DONE 2026-11-20: `load_attested_windows` reads the
      scheduler-settings store file under its own `attestedRepairWindows`
      key (tauri_plugin_store, the D2 mechanism), parse fail-closed
      (`attested_repair_windows_from_value`: missing/wrong-shape/unknown-
      field/any-entry-bad ⇒ whole setting empty + loud warn — the whole
      setting, not per-entry, pinned); the full verdict gate
      `attestation_verdict` = exact walls + `repair_render_sha256`
      (content hash over the spliced per-voice text, deterministic,
      absent-stream marker) + `repair_profile_fp` (strict/no-retry +
      rebuild widths + loaded model id via get_current_model) — Stale
      surfaces both hashes/fps and never splices; `repairs_admissible_
      for_splice` replaces the fold's walls-only filter (legacy class
      splices with an EMPTY allowlist — the class-scoping pin);
      awaiting-attestation logs loudly in the fold; signature threading:
      run_diarization_for_meeting(+_auto) and run_speakers_reset_standalone
      take the parsed entries (command callers read the store; UI-free
      flows pass exactly what the store holds); structural bright-line
      pin moved to tests/production_fixture_brightline.rs (it scans src/
      for the fixture markers and must not self-detect inside src);
      sha2 moved dev-deps → deps. Lib 817 green at -j 2, all targets
      compile.]
- [x] 3.4 RED→GREEN: census outcomes per SEED (enumerated upstream of
      candidate filtering) — `fired+attested`, `awaiting-attestation`,
      `stale-attestation`, `stood-down:<guard>` (manual-row-wins mapped),
      `window-not-built`, `separation-unavailable`, `rebuild-aborted`;
      replace the repair fold's `unwrap_or_default` with the recorded-abort
      path; loud runtime census line for allowlist entries that matched no
      live window this run. [DONE 2026-11-20: `talkover_seed_census`
      assembles one row per talk-over seed — global outcomes (rebuild-
      aborted via the former unwrap_or_default swallow, now a join-failure
      match; separation-unavailable via the batch's new separation_ok
      flag) rank above per-seed stages (coverage ⇒ manual stood-down /
      window-not-built; decode-dropped stood-down for a built window with
      no synthesis back; gate rejections map 1:1 to awaiting/stale with
      the hashes; apply guards map to stood-down:<reason>; applied ⇒
      fired+attested); `format_talkover_seed_census` is the token-only
      TALKOVER-SEED-CENSUS line (info, escalating to warn on
      stale/separation/aborted — the TOKENWALLS precedent); the fold
      runs it whenever talk-over seeds exist (even with zero surviving
      candidates — window-not-built is a real outcome), skips the
      separation spawn when there is nothing to rebuild; `allowlist_
      entries_matching_no_window` surfaces unmatched entries loudly
      (design D2 surfacing duty). Census scope note: the per-seed tokens
      cover the TALKOVER seeds; the legacy phrase-loop fold keeps its
      existing per-window applied/degraded logs (its own requirement's
      coverage). Tests: the exhaustive outcome map (7 shapes + both
      global outcomes), unmatched-entries surfacing, composition test
      updated for the seeds-carrying return. Lib 819 green at -j 2.]
- [x] COMMIT CHECKPOINT: commit the wiring + gate before §4.

## 4. Ear-gate auditability

- [x] 4.1 RED→GREEN (plain `cargo test`; reads the evidence-home fixture
      via `evidence_fixture`, loud skip when absent; negative cases use
      invented walls and synthetic pins — no real pin content transcribed
      into test files): allowlist entry with no matching fixture pin fails
      the gate; promoted per-voice rendered span with no allowlist entry
      fails the gate (scoped to promoted spans via the census record — the
      legacy per-voice path is NOT gated); hash/fingerprint mismatch fails
      the cross-check; overlapping-but-unequal walls and cross-meeting
      matches are unpinned; entries unique by (meeting, walls) — duplicates
      fail; `attested_date` parses YYYY-MM-DD or fails naming the entry.
      [DONE 2026-11-20: tests/repair_attestation_gate.rs — the ledger lint
      (exact-wall same-meeting pin match mirroring the runtime gate's
      semantics; stale render_sha256/profile_fp fails naming the window;
      duplicates by (meeting, walls) fail; strict YYYY-MM-DD lint naming
      the entry; the promoted-span reverse rule over census-recorded
      spans with the legacy path structurally absent from that list) +
      the LIVE cross-check: reads the fixture via evidence_fixture (loud
      skip) and the local scheduler-settings store (env MEETIFY_SCHEDULER_
      STORE override, standard app-config default, fail-closed parse via
      the production attested_repair_windows_from_value). The ledger is
      empty on both sides today (the §5.3 round fills it) — the gate is
      real the moment the first pin+entry land. The live promoted-span
      scan rides §5.2 when the first promoted span exists; the lint is
      pinned with synthetic spans now. 6/6 green.]

## 5. Verification + closure

- [x] 5.1 Full `cargo test` green at `-j 2` including the ear-gate fixture
      lint (no concurrent cargo during live runs). [DONE 2026-11-20: all
      30 test targets ok, exit 0 — lib 819 + integration + doc-tests; the
      ear-gate target ran within the suite; zero FAILED lines.]
- [ ] 5.2 Production Speakers re-run (heartbeat): verify via the
      replay/oracle harnesses — fired spans render per-voice ONLY where
      attested (meeting+walls+hash+fingerprint all matching); every seed
      outcome present in the census (including rebuild-aborted); unpinned
      and stale spans byte-identical to mixture rows; `transcript_sources`
      byte-identity.
- [ ] 5.3 Attestation round: cut before/after clips ONLY for fired spans
      (evidence home or meeting folder), token-only manifest per the
      design's exhaustive field list (walls, clip sha256, clip duration_s,
      seed evidence, outcome token, per-pass token counts, guard-trip
      tokens, retry count, verdict date); one confirmation listen per span;
      dated verdicts become fixture pins (`repair_attestation` kind) +
      allowlist entries (user's word each time). Stands-down on verdict
      keep mixture rows permanently.
- [ ] 5.4 Smoke-spec deliverable decision (AGENTS.md §3, made at proposal
      time): the promotion adds NO new frontend wiring — the visible
      outcome (rows replaced by finer distinct-speaker rows after a
      Speakers run: event → refetch → distinct badges) is already pinned by
      `frontend/e2e/smoke/diarization-speaker-split-persistence.spec.ts`
      (N=3) and `diarization-label-quality.spec.ts` (N=2); the attestation
      gate lives entirely engine-side behind the same Speakers button, and
      the allowlist is deliberately not a dialog (Non-Goal), so engine-side
      selection is structurally invisible to Playwright and is covered by
      the cargo byte-identity gates + ear-gate pins. No new smoke spec;
      this rationale recorded; the covering specs verified passing.
- [ ] 5.5 Before archive: re-read the live spec + design against what
      landed; amend the delta/design first if the implementation evolved
      (the two scenario amendments in the delta apply at archive).
- [ ] 5.6 `/opsx:archive` on the user's go (commits and pushes only on the
      user's explicit word).
