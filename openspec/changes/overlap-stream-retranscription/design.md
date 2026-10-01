# Design: overlap-stream-retranscription

## Hexagonal boundaries

- **Port (reuse)**: the separation pre-pass's `VoiceSeparationPort` —
  unchanged.
- **Transcription seam (new, first caller)**: the diarization pipeline has
  never needed Whisper; this change does. The composition root for Tauri
  commands is `lib.rs`; the closure is built at the adapter command surface
  (`audio/speaker/commands.rs`) over the `whisper_engine::commands::
  WHISPER_ENGINE` static (no `TranscriberPort` exists — port traits are
  deferred) and passed as a `&dyn Fn(&[f32]) -> Option<String>` into the
  synthesis pass. This is the §8 `hexagonal-port-traits` forcing function
  arriving from a real §4 need; the closure is the seam until the deferred
  port-traits change lands. The adapter→adapter import is confined to
  `commands.rs` and recorded as debt in `hexagonal-port-traits`.
- **Use case (new, pure)**: `run_assembly::synthesize_overlap_rows` — pure
  function, no I/O: inputs are the existing rows, the separation votes,
  covered atoms, and per-stream texts; output is the synthesized row list.
  Fully unit-testable with fakes.
- **Adapter (existing)**: `whisper_engine` for stream transcription
  (16 kHz mono in); the hallucination-repair path (language pin, quarantine,
  degenerate-repeat guards) runs on stream text exactly as on live
  transcription output.

## Key decisions

1. **Render-level only.** `transcript_sources` is immutable (spec hard
   invariant). Stream rows exist in the regenerated rendering only; they
   carry their source row ids so persistence and undo flows can trace them.
2. **Both-streams-decisive gate.** Rows are synthesized only when the
   existing margin-gated votes identified BOTH streams (one vote each, ≥
   margin) AND each stream's text survives the hallucination guards
   (non-empty, not quarantined). Anything less degrades to the mixture rows.
3. **Span walls, not token walls (v1).** Stream rows span the overlap span
   itself. Token timestamps on separated audio are DTW-hostile (the S7c
   lesson); word-level splitting inside streams is deliberately out of scope.
   The no-split-sentences invariant holds by construction: one row per
   voice, one badge per row.
4. **Text replacement is honest.** The mixture row's text is REPLACED in the
   render because it is the wrong artifact (interleaved + echo). The echo
   dedup and degenerate-repeat guards still apply per stream (whisper can
   echo a separated stream too).
5. **Cost is bounded**: two whisper inferences per attested overlap span
   (measured: 38 spans meeting-wide, each ≤ a few seconds) on an already-
   loaded model.

## Interactions with existing mechanisms

- **Separation pre-pass**: consumes its port, its `overlap_spans`, its
  covered-atom filter, and its ref-anchored votes verbatim. No new trigger.
- **Wall realignment / voice votes**: synthesized rows enter the render
  AFTER alignment as replacements for the span's rows; they cannot straddle
  boundaries (single span, single voice each), so no new interaction with
  `realign_straddling_atoms`.
- **Ear truth gate**: gains one new assertion class — the S16 window must
  contain two distinct-badge rows whose per-voice text reads in order; the
  gate reads expected needles from the local fixture (real text never
  enters code). It also needs the provenance-scoped duplicate-scan
  exemption (thread A below) or its own overlap scan will fail the render
  this change creates.

## Phrase-loop repair extension (ear round 2026-10-01, S18 window)

The user's clip-01 adjudication proved a second fabrication class the
overlap-mass trigger cannot reach: the August mixture decode invented
"I think it's a good idea" ×3 over 184–192.9s (S18 fixture entry —
nobody says it; real words attested around it), while the RMS profile
shows real speech bursts there. The stutter stand-down keeps those
mixture rows by design, so the fabrication renders. Probe evidence
(`span_decode_probe.rs`, 2026-10-01): fresh decodes with adjusted walls
recover the attested words, and separation over [183.2–194.2] yields
TWO CLEAN PER-VOICE streams matching the ear truth (stream0: Participant B's
full sentence "…Okay, so, well, I mean, it's one or the other. That's
the whole point."; stream1: Participant A's "On hybrid, it would have been
Participant E. I'll figure it out."). The synthesis pipeline can render this
window correctly; only the TRIGGER is missing.

Design: extend the same synthesis path with a phrase-loop trigger.
1. **Candidate**: post-alignment, any non-manual render row whose text
   `is_stuttering_decode` (repeated n-gram ≥2 words, already built for
   stream gates) is a repair candidate.
2. **Window = whole-row absorption**: expand the candidate ±1.3s
   (SEPARATION_CONTEXT_SECS parity), absorb every non-manual row the
   window touches WHOLLY (no straddling pieces — partial coverage
   duplicates the absorbed words into the streams), recompute the window
   as the union, iterate to fixpoint. The gate census word-delta and the
   provenance-scoped duplicate scan police residual boundary duplication.
3. **Synthesis**: the existing machinery — separate(window), per-stream
   decode, margin/RMS/distinct-cluster gates, no-stutter post-gate.
4. **Splice**: replace the absorbed rows with one row per clean stream
   at the window walls (same-wall per-voice shape; sequential exchanges
   render coarser than reality — v1 accepts this, walls not tokens, the
   recorded v1 trade).
5. **Degrade**: manual span inside the window wins (no repair); any
   gate failure → byte-identical keep of today's rows.
Open items: exact pad/expansion constants; whether the fixpoint union
should snap to RMS valleys before absorption; S18 end-to-end
verification via the gate replay before any full re-run.

## Explore-cycle findings (2026-09-29, pre-apply audit)

1. **Whisper seam resolved to a concrete handle**: `whisper_engine::commands::
   WHISPER_ENGINE` (static `Mutex<Option<Arc<WhisperEngine>>>`) with
   `transcribe_audio(Vec<f32>, lang) -> Result<String>`. The closure is built
   from that static at the composition root; an UNINITIALIZED engine (real
   case: probe/test contexts) is the degrade path, not a hypothetical.
2. **Persistence verified, one trap found**: `persist_regenerated_rendering`
   writes `seg.text` (the RENDER text) into fresh-UUID rendering rows and
   takes template metadata from `transcript_sources` keyed by
   `original_id` — so stream text persists as-is. TRAP: a rendering row
   whose `original_id` is absent from `transcript_sources` is silently
   SKIPPED at persist. Stream rows MUST inherit the span's source row id as
   `original_id` (tested in 1.1).
3. **Replacement is already the persist model**: rendering rows are deleted
   and reinserted (fresh UUIDs) on every persist — stream rows are ordinary
   rendering rows; no special replacement mechanics exist or are needed.
4. **Gate snapshot ordering**: the gate pins the replay to a row snapshot
   AND cross-checks the live DB hash — after the render changes, the
   snapshot must be re-pinned AFTER the live persist (task 3.5), not before.
5. **Async bridge — CORRECTED by the panel (finding was wrong as
   written)**: `transcribe_audio`/`decode_with_params` are async for the
   tokio locks on the model context AND three stats locks
   (whisper_engine.rs:841, 985–1016), and the mandated splice point
   (after `resolve_duplicate_clusters`, commands.rs ~970) is OUTSIDE the
   `spawn_blocking` region — the pipeline's only spawn_blocking block ends
   at the join (~796), and the separated streams are moved into it and
   dropped inside. A naive closure at the splice point panics
   ("Cannot block the current thread from within a runtime"). **The pass
   is therefore split along the existing boundary**: per-span synthesized
   CANDIDATES (separation + per-stream decode + per-stream vote, as a
   per-span record `{stream → (cluster, margin, text)}`) are computed
   INSIDE the existing commands.rs:645 `spawn_blocking` and returned
   alongside the engine outputs; the PURE `synthesize_overlap_rows` splice
   runs after the resolver on the async side with no I/O. A new sync
   decode method on `WhisperEngine` (`blocking_read` over
   `current_context`, stats locks skipped or migrated) backs the closure;
   unit-tested on a plain thread. The ear gate replays synthesis inside
   `spawn_blocking` too (its `#[tokio::test]` body is a runtime worker).

## Explore-cycle thread resolutions (2026-09-29, second session)

Resolved in dependency order A → B → C: A unblocks task 1.1, B settles the
quality channel C's scoping stance leans on.

### Thread A (must-resolve before 1.1) — duplicate-scan exemption is provenance, not geometry

The ear gate's duplicate scan (`ear_truth_gate.rs`, overlap_pairs) fails ANY
pair of GROUPS at overlapping walls — and groups are merged turns, not raw
rows, so a stream row can also end up glued to an adjacent same-speaker
neighbor (gap ≤ 3 s). Two stream rows at the same overlap span trip it
either way.

**Resolution**: exempt on provenance, never on wall-offsetting. Verified
mechanics: rows flow through the pipeline as `AlignedSegment`, which
ALREADY carries `original_id` end-to-end (`merge_same_label_fragments`
compares it; there is a `never_crosses_source_rows` test). The exemption
therefore needs NO change to `turns.rs` and no new plumbing in the
pipeline — the gate's `RowRef` projection is what drops the id. The gate
keeps a parallel `original_id` list aligned with its `RowRef` slice;
`TurnGroup.row_indexes` already indexes that slice, so a group's
provenance is the id set of its absorbed rows. Exemption rule at GROUP
level: overlapping-wall groups whose id sets INTERSECT are attested
simultaneous (the stream pair shares the span's source row id — the
both-streams-decisive gate IS the attestation); disjoint id sets at
overlapping walls remain true double-decode suspects and still fail. We
do not "lie about time" (offset walls to dodge the scan). Lands in task
3.3; adversarial test 9 pins both sides.

**New hazard found while pulling (test 10)**: the replay chain runs
`resolve_duplicate_clusters` (production D4) between merge and persist.
The two stream rows share walls and MAY share token-similar text (an echo
or a genuine repeat) — the resolver could classify them as a
re-transcription cluster and DROP a voice. The synthesis pass MUST insert
AFTER that resolver (stream rows are replacements, not a re-decode to
dedup), pinned by a test: the stream pair survives the resolver.

### Thread B — confidence channel is DEAD on arrival; guards carry the weight alone

The planned live probe is cancelled — no probe can rescue this value.
`decode_with_params` (whisper_engine.rs ~:872) computes the returned
"confidence" as `(segment_text.len() / 100.0).min(0.9) + 0.1`: a
TEXT-LENGTH proxy, not whisper.cpp's avg_logprob. Hallucinated fluent
fictions are long, so the proxy is anti-correlated with the exact failure
mode this change must reject. The tuple's `bool` is `is_partial`
(duration < 15 s), not a hallucination flag. The engine's own params
comment already records the real design: gates inside whisper.cpp are
advisory (last decode is still emitted) and "the lane audits text after
decode (audio::hallucination)".

**Resolution**: the composition-root closure stays plain
`&dyn Fn(&[f32]) -> Option<String>` with NO confidence floor; quality is
enforced by the hallucination lane (degenerate-repeat guard, quarantine,
echo dedup) exactly as the both-streams gate already assumed. The pure
function signature is untouched.

### Thread C (USER-DECIDED 2026-09-29) — run everywhere, calibrate by ear, flags never drop

The user ruled: run the talk-over moments machine-wide, then hand them a
sample of clips (up to ~20) to confirm by ear which are genuine talk-overs
and whether the lines read right. Motivation on record: the detector has
dials (trigger mass, vote margins) and may fire too often — over-firing
would produce MORE hallucinated streams, not fixes. Protocol:

1. **Uniform run**: every span meeting the both-streams gate is
   synthesized. No hand-picked list; ear rulings tune the general dials,
   never a per-moment skip list.
2. **Census + word-loss flags**: the gate logs every synthesized span —
   walls, both voices, per-stream text, the trigger mass and vote margins
   that fired it, and a word-loss flag (synthesized rows' word count vs
   the mixture row's). FLAG-ONLY: a flagged span still renders its
   synthesized rows — no automatic dropping or reverting at this
   calibration stage (user explicit).
3. **Clip set (Phase 4)**: up to ~20 moments for the user's ear —
   word-loss-flagged moments first, remainder evenly spaced through the
   meeting (mechanical, not curated). Before sending, the agent reads the
   full rerun transcript against the original and reports whether it makes
   more or less sense (the agent's read is a signal, the user's ear is the
   judge).
4. **Graduation bar unchanged**: S16's fixture needles stay the regression
   pin; the 33.2–38.5 window proves no synthesis outside real overlaps.

Cost bound: worst case 2 inferences × 38 spans ≈ 76 short inferences on an
already-loaded model.

## Security / trust boundaries

Stream text is untrusted model output: it passes the same schema validation
and hallucination guards as live transcription before touching the render.
No LLM in this path. The separator's output never reaches persistence
(audio only becomes text via Whisper; embeddings never leave the process).

## Pre-implementation adversarial panel (2026-09-29, 5 reviewers) — folded decisions

Five read-only reviewers (architecture, test coverage, security/PII, spec
fidelity, data science) attacked the artifacts. Convergent and singular
findings, folded as follows. Blocking measurement first:

- **P0 — RESOLVED by the task 0.1 dry-run census (2026-09-30, probe
  committed, env-gated + token-only)**: S16 does NOT fire at the current
  dials — not because of the mass bar (the window's mass peaks 0.726,
  above 0.5) but because mass ≥ 0.5 never SUSTAINS the 0.4 s minimum
  (longest run 0.270 s; 0.405 s at a 0.3 bar). Sweep (mass, duration →
  fired spans, S16): (0.5, 0.4) → 38, no; (0.5, 0.3) → 61, no;
  **(0.5, 0.25) → 75, FIRES**; (0.5, 0.2) → 87; (0.5, 0.1) → 109;
  (0.4, 0.4) → 62, no; (0.4, 0.3) → 85; (0.3, 0.4) → 80. Of the current
  38 fired spans: 22 both-decisive-distinct (span-level proxy for
  synthesis — production atom votes may reject more), 13 margin rejects,
  10 same-badge collapses (the distinctness gate is load-bearing), 2
  collapsed-stream energy splits. **USER RULED (2026-09-30)**:
  `OVERLAP_MIN_SPAN_SECS` 0.4 → 0.25 (mass bar stays 0.5) — the smallest
  change that catches S16; the mass bar keeps its crosstalk-discriminator
  role; the calibration clip set will show what the shorter window admits.
  Dial probe reuses the gate's frame-mass cache so re-runs are cheap.

Design decisions pinned by the panel:

1. **Language (arch + DSP + tests converged)**: the stream decode uses the
   meeting's resolved language — the concrete code when the user
   preference names one, else the same resolution the mixture rows used.
   Never `auto-translate` (the global default would render Spanish
   meetings as English stream rows), never per-stream auto-detect on
   2–3 s clips, never the strict lane's `en` fallback.
2. **Stream identity + distinct badges (arch + DSP + tests converged)**:
   votes are bare `(start, end, cluster)` tuples with stream order
   non-contractual — badges cannot be derived from them. The vote record
   is extended to a per-span record `{stream → (cluster, margin, text)}`,
   and the gate requires the two streams' clusters to DIFFER; a same-badge
   collapse (Conv-TasNet returns voice + residue) degrades to the mixture
   render. The real same-shape hazard is consolidation merging same-badge
   rows at negative gap (turns.rs gap ≤ 3 s, negative for identical
   spans) — concatenating two voices under one badge.
3. **D4-resolver hazard re-pointed (arch)**: `duplicate_pair` never merges
   overlapping spans by construction — the resolver hazard is structurally
   impossible; test 10's ordering stays (costs nothing) but the pinned
   adversarial energy moves to the consolidation case (test 13).
4. **original_id selection for multi-row spans (arch + DSP)**: a span can
   cover atoms of two source rows; each stream row inherits the id of the
   source row with maximum covered-atom overlap (tie → earliest start),
   pinned by test. Replacement scope (a row straddling the span walls)
   keeps its out-of-span remainder or is replaced whole — pinned, no
   silent word loss.
5. **Census contract (DSP + tests + security converged)**: the census
   covers ALL trigger-fired spans (synthesized or rejected) — trigger
   mass, per-stream best/second similarities and margins, pre-
   normalization RMS ratios, outcome — so retuning has the near-miss
   distribution, not just precision. This requires
   `separated_stream_voice_votes` to stop discarding margins. Recorded
   census artifacts are TOKEN-ONLY (walls, ids, margins, word/char counts,
   sha256 of each stream's text); verbatim text is terminal-only
   (env-gated) or in the evidence home — the runner script's output file
   in the change folder must never carry meeting text (security P0), and
   `openspec/changes/**/gate-runs/` gets a .gitignore entry + pre-push
   pathspec.
6. **Whisper stream profile (DSP)**: deterministic decode — pinned
   language, greedy (reuse the strict lane's param shape), temperature 0
   (no noise-amplifying fallback ladder on bleed-heavy streams), no token
   timestamps (rows use span walls). Below-1 s spans may return empty —
   the non-empty guard degrades them, census logs it.
7. **Eligibility floor (DSP)**: a stream is eligible only if its
   PRE-normalization span RMS is ≥ a stated fraction of the clip's RMS
   (target 0.1); SEPARATION_MIN_WINDOW_RMS evaluated post-normalization
   means "digitally silent", not "no voice" — normalization amplifies
   near-silence 100–1000× into the hallucination regime. Optional
   absolute-cosine floor on votes.
8. **Word-loss statistic (DSP)**: duration-normalized rates
   (words/sec over the span, flagged both directions — too low = lost
   words, too high = hallucination); raw count vs the SUM of replaced rows
   is secondary. The mixture row's count is echo-inflated, so a raw ratio
   would bias the clip set toward non-defects.
9. **Cross-stream duplication flag (DSP)**: same utterance leaked into
   both streams (likely near span edges) is a distinct failure the
   provenance exemption would positively pass; census flags ≥3-token
   contiguous overlap covering ≥80% of the shorter text. Flag-only.
10. **Manual rows (arch + tests)**: a span containing a surviving
    manually-labeled row does not synthesize — midpoint suppression would
    otherwise eat BOTH fresh stream rows on the next run (the un-labeled
    voice vanishes). Manual wins; synthesis degrades for that span.
11. **Byte-identity comparators (spec + tests)**: `transcript_sources`
    via the source_hash digest; the degrade render via the structural
    signature (count/text/span/badge, generated ids excepted — fresh UUIDs
    make id-inclusive identity impossible by construction).
12. **Gate needs a real engine (arch)**: the gate process has
    WHISPER_ENGINE = None → degrade → the S16 assertion could never pass;
    task 3.3 constructs the engine explicitly (precedent: the live
    speaker tests) and the census records the model name.
13. **speaker_source (arch)**: persist hardcodes `'auto'`; task 3.2 is
    rescoped to "stream rows persist as `'auto'`" — a new SpeakerSource
    variant would touch every row's persistence and both label predicates,
    out of scope for v1.
14. **Threshold provenance (DSP, on record)**: margin 0.05 is the
    production mixture bar applied unchanged (measured stream margins
    0.135–0.29 give 3–6× headroom); trigger mass 0.5 + 0.4 s was chosen
    for cost, never validated for false-positive rate. Inherited, not
    stream-calibrated — the census + ear protocol is what calibrates them.
15. **Pre-existing public verbatim needles (security, out of scope
    here)**: hardcoded dialogue needles already on origin/main
    (ear_truth_gate.rs, alignment.rs, commands.rs) — do NOT extend the
    pattern (S16 needles come from the local fixture); a dedicated scrub
    change follows.

## Live-gate findings (2026-09-30, first full replay — task 3.3 verification)

The env-gated ear gate ran the FULL synthesis replay against the real
meeting (75 candidates at the ruled dials; real Whisper decodes on both
streams of every span). Results, and the one open design question:

1. **The machinery works end to end.** All 75 spans separated; both streams
   decoded with real text on 61; identities decisive and DISTINCT on those
   (margins 0.038–0.43); the census, guards, degrade path and scan
   exemptions all behaved. Every gate that failed, failed honestly
   (byte-identical degrade; the gate itself stayed green via the S16
   amendment waiver).

2. **BLOCKING FINDING — the row-coverage replacement rule fires (almost)
   never.** 0 of 61 healthy candidates synthesized. Cause: the replacement
   rule replaces only rows the span covers ≥ SYNTH_ROW_COVERAGE (50% of the
   ROW's duration) and requires such a donor row. But real crosstalk spans
   (0.25–0.5 s at the ruled dials) are almost always a MINORITY of their
   containing mixture row (rows are sentence-scale; crosstalk is
   backchannel-scale) — so no row is ever ≥50% covered and every span
   degrades. The rule assumed span/row geometry agreement that does not
   hold. The gate did exactly its job: this would otherwise have been
   discovered as "the feature silently never fires".

3. **S16 geometry reality**: at the ruled dials (mass 0.5, duration 0.25 s)
   the trigger fires only the exchange's TAIL — span [1056.97–1057.24]
   (0.27 s). The exchange body (1055.5–1057.5) hovers at pyannote overlap
   mass 0.16–0.26 (the fixture's own corroboration) and never crosses the
   0.5 bar for long. Even a perfect replacement rule therefore cannot make
   the S16 EXCHANGE per-voice without lowering the mass bar to ≈0.2–0.3
   (span counts rise accordingly; the census quantifies the cost).

**DECIDED (user, 2026-09-30)**: (b) + a lowered bar — span-slice surgery
with a persisted parent link, and the mass bar moves to 0.3. The user's
explicit requirement: the split pieces MUST carry a parent identifier so
downstream analysis (the AI summarizer) can reconstruct the pre-split
sentence — implemented as the `synth_parent` column (head/tail pieces link
their own source row; stream rows link the donor), while head/tail pieces
stay merge-eligible so consolidation re-heals them into readable turns.
Mass bar 0.3 (the moderate option): real exchanges peak at 0.16–0.26, so
0.5 structurally never fired; 0.3 admits ~80 spans meeting-wide and the
clip calibration measures what it admits.

## Adversarial tests (RED before GREEN)

Replacement scope (pinned by task 1.1, per the delta spec's "stated in the
design" requirement): a mixture row is replaced iff the covered-atom time
overlap is at least half the row's duration (`SYNTH_ROW_COVERAGE`, mirroring
run_engine's SEPARATION_ATOM_COVERAGE); such rows are replaced WHOLE — an
out-of-span remainder's words leave the render, and the word-loss diagnostic
is the honest accounting of that loss (the calibration clip set judges
whether it is acceptable). A row under the bar survives untouched — no
partial-row surgery, no silent loss. The source row whose covered overlap is
largest (tie: earliest start) donates its `original_id` to both stream rows;
if no row reaches the bar, synthesis degrades — an id is never fabricated.

1. Silent stream (RMS-floor passed but no speech) → whisper returns empty →
   NO synthesis; mixture rows survive.
2. Hallucination stream (whisper echoes a nonexistent sentence) → caught by
   the existing degenerate-repeat guard → NO synthesis for that stream.
3. Margin miss on either stream → NO synthesis (mixture rows survive).
4. Span clamped at recording edges → no panic, no synthesis.
5. Immutability: a full run leaves `transcript_sources` byte-identical while
   the regenerated rendering contains the stream rows.
6. Byte-identical degrade: separation model missing / whisper unavailable →
   render identical to today's (existing channel).
7. Persistence round-trip: stream rows persist with their per-stream badge
   and survive a refetch (DB-layer verification, terminal-only rule).
8. Gate: S16 window renders two distinct-badge in-order rows (needs a real
   loaded engine in the gate process, decision 12); no synthesis outside
   overlap spans, asserted mechanically — every synthesized row's span is
   covered by an overlap span (covered-atom property, synthetic-subset
   assertable without audio).
9. Duplicate-scan exemption is provenance-scoped (thread A): two
   overlapping-wall groups whose absorbed-row id sets intersect (stream
   pair sharing the span's source id) pass the scan; a disjoint-id
   overlapping-wall pair still fails it.
10. Stream pair survives `resolve_duplicate_clusters`: synthesis inserts
    after the D4 resolver. (Panel correction: the resolver never merges
    overlapping spans by construction, so this ordering is belt-and-
    braces; the pinned hazard is consolidation — test 13.)
11. Word-loss flag is diagnostic only: a span whose synthesized rows lose
    words vs the mixture row is flagged in the census and still renders
    its synthesized rows — the flag never drops or reverts anything.
12. Oversized span: a minutes-long overlap span (no MAX guard exists
    today) degrades or is bounded by an OVERLAP_MAX_SPAN_SECS guard —
    never two unbounded inferences and a render-hostile mega-row.
13. Same-badge collapse: both streams voting one cluster → NO synthesis;
    stream rows survive consolidation as atoms (a stream row is never
    re-merged into an adjacent same-speaker turn).
14. Language pin reaches the decode: a fake engine records the language
    argument; the meeting's resolved language (never auto-translate)
    arrives for every stream decode.
15. Manual row wins: a span with a surviving manual row does not
    synthesize, and the OTHER voice's fresh row is not suppressed by
    midpoint suppression on the next run.
16. Cross-stream duplication: near-identical texts on the two streams are
    flagged in the census (suspected separator leak) and still render —
    flag-only.
17. Census contract: a synthesized span's census record carries walls,
    both voices, token-only text data (counts + sha256), trigger mass,
    per-vote margins, word-loss and cross-stream flags — a green
    implementation missing any field fails the test.

## §3 smoke-spec decision

The synthesis surfaces through the existing transcript-render flow; the
Speakers diarization run itself is not Playwright-drivable (real models +
recording, same rationale recorded by subturn-voice-attribution and
overlap-separation-prepass 3.3). The render change is pinned by the offline
ear gate (assertion class above) + the persistence round-trip test; if apply
touches a Tauri command surface, the smoke spec is added in that task per
the standing rule.
