## Context

The previous repair attempt (`overlap-repair-net-worse-guard`, archived 2026-11-17) proved on measurement that separated-stream decodes recover the true talk-over word order (the validated 32.69s window), then dropped the promotion: its automated acceptance bar — a per-badge token-conservation net-worse guard — could not separate a validated render from a paraphrased one (the mangled shape scored HIGHER; separation word-drop and cross-stream bleed noise dwarfs the 2–3-token fluent-substitution signal). The archived record concludes the paraphrase defect is meaning-level; the only oracle is the ear. The user's ruling (2026-11-18): decode reliability first, then ear pins.

What survives live and unchanged: the repair-window machinery (build/splice/absorption, floors), the separation adapter (probe-validated in-domain 2026-09-28), per-voice row rendering with margin-gated votes, hallucination guards, byte-identical degrade, `synth_parent` links, the census/replay/persistence-oracle harnesses, the ear-truth gate with the waiver router (pin-scoped, hard pins never waivable), and the conservation statistic as measurement instrumentation only. Rebuild material: the archived change's `rebuild-notes.md` (API sketches: `SpanDecodeEvidence`, `scramble_seed_spans`, `stream_decode_rate_fiction`, probe envs) and the three kept window shapes.

Panel-hardened (5-reviewer pre-implementation panel, 2026-11-18): their findings are folded below; the two rejected demands are recorded with reasoning in the Risks section.

## Goals / Non-Goals

**Goals:**
- Separated-stream decodes inside rebuilt repair windows recover verbatim text reliably, measured offline END-TO-END (decode through builder+splice) against the attested window shapes BEFORE any render ships.
- Every candidate overlap flag is recorded token-only (census) so detector precision is measured, not assumed.
- Qualifying talk-over spans promote through the live repair builder and render per-voice ONLY where the user has ear-attested that window's CONTENT (fail-closed, bright-line-safe, meeting-scoped, staleness-checked).
- The user's dated per-span verdicts become permanent fixture pins the ear gate enforces.

**Non-Goals:**
- No new separation model or checkpoint work (the Conv-TasNet port passed its Phase-2 quality probe; swapping models is out of scope).
- No changes to the class-E floors or any detector dial — dial moves happen only on the user's dated rulings (standing ear-calibration hold).
- No summarizer, export, or UI changes. The attestation allowlist is a settings-file record, not a dialog.
- No attempt to resurrect an automated net-worse acceptance bar — that premise is falsified and recorded.
- The LEGACY per-voice render path (margin-gated spans that qualify without promotion) is unchanged by this change; extending the attestation gate to it is opportunistic when next touched (recorded scope decision).

## Decisions

**D1 — Automation rejects, never accepts; acceptance = dated ear pins bound to content.**
The token-conservation guard is dead on measurement (archived record). The automated layer keeps only REJECTION duties it already has: rate-fiction/stutter stand-down, hallucination guards, margin-gated distinct-voice requirement, manual-row-wins, byte-identical degrade. Acceptance of a promoted render is the user's dated ear verdict. Because walls alone do not identify the thing the ear judged (a model or retry-profile change re-decodes the same walls into different — possibly paraphrased — text), the pin binds CONTENT: the attestation entry carries the sha256 of the spliced per-voice text plus a decode-profile fingerprint (model id + retry variant + rebuild-width class). Any mismatch at splice time ⇒ treated as unpinned ⇒ mixture rows + `stale-attestation` census outcome with both hashes logged loudly. Decode-affecting changes invalidate pins mechanically — the honest cost of "nothing unverifiable ships".

**D2 — Attestation gates the render at runtime via a token-only, meeting-scoped allowlist; production never reads the fixture.**
`attestedRepairWindows` lives in the app's local Tauri store (`settings.json`, the scheduler-settings mechanism — local-only, no sync path). Each entry carries EXACTLY `{meeting_id, start_s, end_s, attested_date, render_sha256, profile_fp}` — no additional field SHALL ever be added (pinned by a `deny_unknown_fields` deserialization test); the setting is never exported, synced, or committed. Walls+date+hashes are token-only coordinates, the same class the archived record publishes. The splice requires, within the SAME meeting: exact wall equality, matching `render_sha256` and `profile_fp`, and every rejection guard passing. A parse failure or malformed entry ⇒ the setting is treated as empty for that run (fail-closed) with a loud warning. The fixture-side pin is an ear-truth entry of kind `repair_attestation` whose params equal the allowlist entry (meeting, walls, date, hash, fingerprint); the ear gate cross-checks the two sides — a TEST-side gate only, run under `cargo test` via the evidence-home loader; production code SHALL NOT read the evidence fixture. The gate's reverse rule (rendered per-voice span must have an entry) is scoped to PROMOTED spans, identified via the census record — the legacy per-voice path keeps its own requirement's coverage. Entries are unique by (meeting, walls); duplicates fail the fixture lint. Runtime surfaces stale state loudly: allowlist entries that matched no live window this run, and `stale-attestation` outcomes, are census lines + run-log warnings, not env-gated-only.

**D3 — Decode reliability is a measured lever ladder; probe and runtime share one wall builder.**
The offline stage compares legs on the kept attested shapes: leg A = widened rebuild (windows built by the LIVE `repair_windows_from_seeds` construction — the same code that produced the recorded 30–32.69s shapes; the earlier "~9–12s" figure was a decode-context floor, not a rebuild width, and is retracted as a shipping claim) under the existing strict profile; leg B = leg A plus the named retry variant: a cloned decoder context with no-condition enabled (`set_no_context(true)` on a clone — never mutation of the shared params object). The selection rule is FROZEN before measurement: pass 1 stands unless it tripped a guard AND the alternate pass clears every rejection guard; at most one alternate; both passes' evidence recorded. The comparison operator is FROZEN before measurement: the ear gate's OWN pin semantics (AMENDED at the 2.4 tick, 2026-11-18 — the drafted "Unicode NFC then exact token-sequence equality" is retracted: the fixture pins are needles at attested multiplicity, not full sequences; the frozen operator is `norm_tokens` (detokenize → lowercase → non-alphanumeric → space) + contiguous non-overlapping run counting + joined scope + `got != pin.count`, the gate's own matcher, which the probe mirrors verbatim). The probe drives the FULL builder+splice path (not bare decodes) so its verdict covers assembly, badges, and census outcomes end-to-end. GREEN bar: each attested shape recovers its attested sequence; a shape that no leg recovers is a recorded STOP — the user re-plans before wiring ships. If leg A recovers everything, NO retry ships (the retry is conditional on the measurement). The retry profile is structurally unreachable from the general chunk path AND the mass overlap pre-pass (two structurally distinct decode profiles; the pre-pass keeps the plain profile — its byte-identity is pinned by test, since a retry there would change renders outside any repair window with no allowlist gate).

**D4 — Promotion rebuild from the archived sketches, through the live builder, class-scoped, with attestation-driven seed visibility.**
Two seed routes (census finding 2026-11-18: the attested talk-over moments — S16 seed 0.40s, the 192.9s seed 0.84s — stand down BEFORE separation at `MIN_SYNTH_DURATION_SECS` = 1.5s, so a decode-tripped-only seed definition can never rebuild them; the route that fixes the user's actual complaint must see sub-floor spans):
- **Decode-tripped seeds** (as drafted): a duration-passing span whose separated decode trips rate-fiction or stutter — cheap (~2 candidates/run pass the duration floor).
- **Attested-window seeds**: a trigger-fired span whose walls fall within an `attestedRepairWindows` entry rebuilds its window REGARDLESS of the duration floor — the attestation IS the prior ear evidence that the span is a real talk-over needing repair, and the hash-checked splice keeps it fail-closed. The duration floor's GPU purpose is preserved (rebuilds happen only for the handful of attested windows + ~2 decode-tripped seeds), and its fiction-suppression purpose is preserved by the guard set + content-hash pinning. The rebuild drives the full live builder at the attested wide-window walls (the margin evidence exists only at that shape — the narrow span's span-level margins read ~0.01–0.05, far under the vote bar, while the probe-validated wide-window margins read 0.135/0.147).
`LoopRepairCandidate` gains a seed-class discriminator; the attestation gate binds ONLY to the talk-over seed class — live phrase-loop repairs keep their ear-ruled status and splice WITHOUT an allowlist entry (pinned by test), so gating does not silently disable the S18-validated phrase-loop path. The decode-tripped class's promotion explicitly SUPERSEDES the 2026-10-07 ruling that fiction candidates seed no repair (`commands.rs` seed filter amended in task 3.1) — the attestation gate is the replacement safety the ruling lacked; the attested-window class is itself the user's per-window ruling. Window rules: two seeds in one window union deterministically; the UNIONED interval re-runs the manual-row and synth-atom checks; a refused union falls back to the individual windows if individually clean, else `window-not-built`. The repair pass returns a PER-SEED outcome (not a silent `Vec` with `unwrap_or_default`); a rebuild panic/join error is a recorded `rebuild-aborted` outcome, not an empty continuation. Census outcomes, enumerated from SEEDS upstream of candidate filtering: `fired+attested`, `awaiting-attestation`, `stale-attestation`, `stood-down:<guard>` (manual-row-wins mapped here), `window-not-built`, `separation-unavailable`, `rebuild-aborted`. These extend the existing trigger-fired census record (one census, not a parallel one).
Measured 2.4 finding (2026-11-18, `probe-record-2-4.md`): the attested walls MUST align to row
boundaries on the hash-pinned render. The probe drove the recorded walls [1024.61,1057.30] — cut on
the 2026-11-16 live geometry — against the then-current render, where the S16 moment sat mid-way
inside a 32s mega-row: the in-window content recovered on leg A, but the absorb-whole splice ERASED
the mega-row's outside-window words (17.12s east) carrying needle occurrence #2 (want=2 got=1). At
runtime D4's `render_sha256` gate fails exactly this scenario closed (`stale-attestation` →
byte-identical mixture rows, no regression) — the probe measured what that gate prevents,
confirming the content-hash design. Consequence recorded as a rule: attestation entries are cut
ROW-ALIGNED on the render they hash-pin (the §5.3 round derives entry walls from the fired
candidate's row-aligned walls, never hand-drawn). The row-aligned re-cut of the then-current
geometry was measured 2026-11-18 and failed differently (separation collapses on that
composition: stream0 4 tokens vs 44, both needles absent — probe record
addendum); root cause traced in `mega-row-investigation.md`: the region rendered as one
32s token-wall-less mega-row spanning two source chunks.

UNBLOCKED 2026-11-20 (token-walls change landed 2026-11-19, archived
`2026-11-19-render-token-walls`; its `handoff-note.md` carries the measured
geometry): the render is now wall-bearing (census rows=269 walled=194
null=75 violations=0), the mega-row carries a real per-word wall list, and
the wall-split straddle policy dissolves the absorb-whole trap. The S16
re-cut on the wall-bearing render is MEASURED (handoff probe, commit
624d66b3): window [1024.75,1056.50] (~31.75s ≤ the 35s chain cap) — WEST
edge 1024.75 row-aligned with the NULL-walled row's end (that row is NOT
absorbed; the overlap predicate excludes a touching row; its 42s of
outside-window text cannot be erased), EAST edge 1056.50 the token boundary
nearest the attested wall 1057.30 on the walled mega-row (occurrence #1
ends 1056.50 inside the window; occurrence #2 spans [1058.41,1059.17] and
survives untouched in the east piece under its real walls). The two
NULL-walled rows inside the window are absorbed whole and REPLACED by
stream rows — their being wall-less is irrelevant to the splice. Baseline
pins hold on the new render (S16 2/2, S18 7/7); builder outcomes unchanged;
separation margins at the recorded shape identical to 2026-11-18
(0.5777/0.2167). Entry walls for S16: [1024.75,1056.50] (derived — west
row-aligned, east token-aligned — never hand-drawn, per the rule above).
Two carried obligations from the handoff: (1) `render_sha256` SHOULD cover
the wall payload — walls are part of the render's identity and
regeneration re-derives them; (2) a decode re-measurement at the re-cut's
own walls is owed before the S16 entry can fire. **Measured 2026-11-20
(probe record addendum, sha 4cdafb01…): the re-cut RECOVERED=NO at its own
walls.** The rebuild decodes cleanly (stream_tokens=[44, 39], guards
clean, splice ok, leg B identical — the wiring is measured correct) but
the spliced render FAILS the ear gate's own pins: occurrence #1 want=1
got=0, occurrence #2 want=2 got=1 — the handoff's predicted needle
coverage is falsified by the decode+pin evaluation. **STOP stands; the
S16 entry cannot be cut on this geometry** — the user re-plans before any
S16 entry ships (the runtime gate never admits one meanwhile). Two open
re-plan questions recorded: the census trigger [1056.90,1057.30] sits
east of the re-cut window (the attested route seeds on trigger
containment — this entry could not fire even with pins held), and the
east-edge token alignment (1056.50 vs attested 1057.30) interacts with
the pin window's joined scope. West
rows [982.17,1024.75]…[1027.86,1032.31] stay NULL-walled (source chunk
[1010.07,1032.31] fails the validator's backtrack tolerance) — a dial-
adjacent question for the user, never a unilateral move; no §3 path needs
a NULL-side split.

**D5 — Census before render changes; review produces a dated artifact.**
Task 1 extends the existing `overlap_census_probe.rs` (DRY) rather than starting a new probe, and enumerates from `engine_out.overlap_spans` — which is computed independent of enrollment, so detector precision is measurable on meetings without enrolled references. Every candidate flag is recorded token-only (wall position, overlap mass, duration, floors verdict, counts+sha256). The user reviews the census; the review produces a dated token-only note in the change folder (counts + ruling) that is a hard prerequisite for opening §3.

**D6 — Bright-line conventions inherited in full, with exhaustive record schemas.**
Probes env-gated (`MEETIFY_LIVE_DIAG`, `#[ignore]`, loud evidence-home skip); verbatim only under `MEETIFY_RENDER_PRINT` (restated at the probe's mismatch-diagnostics step); clips written ONLY under the evidence home or the meeting folder; `cargo test -j 2`; no concurrent cargo during live runs. Record field lists are EXHAUSTIVE:
- attestation manifest (per fired span): walls, clip sha256, clip duration_s, seed evidence (overlap mass, duration_s, floors verdict), outcome token, per-pass token counts, guard-trip tokens, retry count, verdict date.
- decode record (per pass): walls, per-pass token counts, guard trips, retry count, decode sha256.
- census row: wall position, overlap mass, wall duration_s, floors verdict or seed outcome, counts, sha256.
These field lists are exhaustive: no record field SHALL ever carry transcript or decode text, token sequences, per-word timings, embeddings, or audio; growth is limited to counts and ratios keyed by wall coordinates. Sinks: records land as token-only markdown in the change folder (the `offline-evidence.md` precedent); runtime census outcomes land in the existing production census record + run log.

## Risks / Trade-offs

- [Content-hash binding vs decode nondeterminism: whisper's temperature fallback ladder runs even under strict params, so a re-decode can differ without any engine change] → the mismatch path is fail-closed and LOUD (`stale-attestation` census line + both hashes logged); the user re-attests with one listen. Accepted over the alternative (no hash) which re-opens the silent-stale-splice hole.
- [Wall drift (alignment jitter moves walls 1ms) silently un-pins an attested window] → exact equality fails closed; the unmatched-entry runtime census line surfaces it loudly. Wall normalization (grid rounding) rejected for now: it widens the match without adding evidence.
- [Paraphrase class survives every leg on some future span] → stage-1 STOP gate catches it on the attested shapes; at runtime the pin binds content, so an unattested decode never splices; the ear-pin flow rules new spans per-listen.
- [Retry lever may prove unnecessary (leg A sufficient)] → the ladder measures first; the retry ships only if the measurement demands it.
- [Gate mis-scoping could disable live phrase-loop repairs] → class-scoped gate + a test pinning that phrase-loop windows splice without allowlist entries.
- [Detector precision may measure poorly] → harmless while stands-down dominate (mixture rows keep correct text); the census informs the floor conversation but no dial moves without the user's ruling.
- [GPU/wall-clock] → census first (cheap, CPU), offline probes on three shapes, ONE production Speakers re-run with heartbeat; the replay/oracle harnesses verify mechanically after.
- [Rejected panel demands, recorded] — (a) a single-writer tool emitting pin+allowlist atomically: YAGNI for a one-user flow; the dated verdict updates both by hand and the gate cross-checks mismatches. (b) a golden decode-output fixture for the general path: model-dependent goldens are brittle and need the GPU in tests; structural reachability isolation + param-cloning tests give the same guarantee without them.
