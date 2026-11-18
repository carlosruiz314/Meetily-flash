## 1. Candidate census — measure the flagging before touching anything

- [ ] 1.1 Extend `tests/overlap_census_probe.rs` (DRY — no parallel probe;
      env-gated, `MEETIFY_LIVE_DIAG`, loud evidence-home skip, token-only):
      enumerate every overlap-span candidate flag from
      `engine_out.overlap_spans` (computed independent of enrollment, so
      un-enrolled meetings measure too) with wall position, overlap mass,
      duration, floors verdict (fires / stood-down), sorted by position;
      counts + sha256 only.
- [ ] 1.2 Review the recorded census with the user (STOP gate); the review
      produces a dated token-only note in the change folder (counts +
      ruling) — a hard prerequisite for opening §3.
- [ ] COMMIT CHECKPOINT: commit the census record + note before §2 (the
      dropped predecessor lost everything; §7's rule).

## 2. Decode reliability — offline, red-first, measured lever ladder

- [ ] 2.0 RED (structural): the retry decode profile is unreachable from
      the general chunk path AND the mass overlap pre-pass (no call edge;
      the pre-pass keeps the plain profile); the retry variant uses a
      CLONED decoder context (`set_no_context(true)` on the clone), never
      mutation of shared params.
- [ ] 2.1 RED (plain `cargo test`, invented generic tokens): the FROZEN
      selection rule — pass 1 stands unless it tripped rate-fiction or a
      stutter/hallucination guard AND the alternate pass clears every
      rejection guard; at most one alternate; both passes' evidence in the
      decode record (walls, per-pass token counts, guard trips, retry
      count, decode sha256).
- [ ] 2.2 RED: malformed/empty decode handling — a rebuilt stream decoding
      empty must not replace non-empty mixture rows (stand down, record).
- [ ] 2.3 RED: union-window rules — two seeds union deterministically; the
      UNIONED interval re-runs manual-row and synth-atom checks (RED test
      for the straddling-manual-row union); refused union falls back to
      individual windows if individually clean, else `window-not-built`.
- [ ] 2.4 Offline lever-ladder probe on the three kept window shapes
      (env-gated, evidence-home fixtures, token-only record): the probe
      drives the FULL builder+splice path; leg A = live rebuild width +
      existing strict profile; leg B = leg A + the named no-condition
      retry; comparison operator FROZEN before measurement (NFC, lowercase,
      punctuation stripped, whitespace collapsed, exact token-sequence
      equality). GREEN bar: each attested shape recovers verbatim on the
      measured minimal leg; unrecovered shape = recorded STOP (user
      re-plans; no wiring ships). Mismatch diagnostics print verbatim only
      under `MEETIFY_RENDER_PRINT`; the STOP record stays token-only.
- [ ] COMMIT CHECKPOINT: commit the probe record + chosen leg before §3.

## 3. Promotion wiring — rebuild from the archived sketches

- [ ] 3.1 RED: seed = a rate-fiction or stutter talk-over span that REACHED
      separation (duration-floor carve-out excluded); `LoopRepairCandidate`
      gains the seed-class discriminator; negated-reachability test first
      (no promotion reachable from the repair fold until wired); RED test
      that an overlapping, token-similar, different-badge span pair
      SURVIVES `resolve_duplicate_clusters` (pins the existing
      overlapping-pair guard against regression); the fiction-candidate
      seed filter is explicitly amended for the talk-over class — design
      D4's supersession of the 2026-10-07 ruling recorded.
- [ ] 3.2 GREEN: route the promoted window through the live repair-window
      builder (separation + per-stream decode + per-voice splice with
      `synth_parent` links); all existing rejection guards apply unchanged
      (margin-gated distinct-voice votes, hallucination guards, RMS
      eligibility, duration floor, w/s ceilings, manual-row-wins,
      byte-identical degrade, `transcript_sources` byte-identity).
- [ ] 3.3 RED→GREEN: the attestation gate — `attestedRepairWindows` in the
      Tauri store (`settings.json` scheduler-settings mechanism): entries
      exactly `{meeting_id, start_s, end_s, attested_date, render_sha256,
      profile_fp}`; `deny_unknown_fields` (unknown field ⇒ treated as
      empty + loud warning); malformed setting ⇒ empty for that run;
      same-meeting exact-wall equality; hash + fingerprint must match
      (mismatch ⇒ `stale-attestation`, both hashes logged); unpinned ⇒
      mixture render byte-identical + `awaiting-attestation`; production
      never reads the evidence fixture (structural test); phrase-loop
      windows splice WITHOUT an allowlist entry (RED test pins the
      class-scoping).
- [ ] 3.4 RED→GREEN: census outcomes per SEED (enumerated upstream of
      candidate filtering) — `fired+attested`, `awaiting-attestation`,
      `stale-attestation`, `stood-down:<guard>` (manual-row-wins mapped),
      `window-not-built`, `separation-unavailable`, `rebuild-aborted`;
      replace the repair fold's `unwrap_or_default` with the recorded-abort
      path; loud runtime census line for allowlist entries that matched no
      live window this run.
- [ ] COMMIT CHECKPOINT: commit the wiring + gate before §4.

## 4. Ear-gate auditability

- [ ] 4.1 RED→GREEN (plain `cargo test`; reads the evidence-home fixture
      via `evidence_fixture`, loud skip when absent; negative cases use
      invented walls and synthetic pins — no real pin content transcribed
      into test files): allowlist entry with no matching fixture pin fails
      the gate; promoted per-voice rendered span with no allowlist entry
      fails the gate (scoped to promoted spans via the census record — the
      legacy per-voice path is NOT gated); hash/fingerprint mismatch fails
      the cross-check; overlapping-but-unequal walls and cross-meeting
      matches are unpinned; entries unique by (meeting, walls) — duplicates
      fail; `attested_date` parses YYYY-MM-DD or fails naming the entry.

## 5. Verification + closure

- [ ] 5.1 Full `cargo test` green at `-j 2` including the ear-gate fixture
      lint (no concurrent cargo during live runs).
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
