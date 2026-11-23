## Why

A repair window can render a synthesized fragment the mixture decode contradicts. In the certified S16 window (fork issue #7, probe-record addendum 5 of `talkover-word-order-repair`): the moment has three voices but the separator produces two streams, so the third speaker's clipped second talkover occurrence decoded as a one-word fiction under the wrong stream badge at the window tail. The user's acceptance metric is "fewer hallucinations = better" — but the panel's applied-science review established that at a talkover tail, a cross-voice textual difference is ALSO the expected signature of successful separation (both voices carrying through the overlap), so an unmeasured trim rule could delete genuine words. The intervention must therefore be measured before it ships anywhere.

## What Changes

- Add a tail-provenance guard to the repair pipeline as a PRE-VERDICT synthesis-level pre-pass (NOT inside the splice — the attestation content hash binds the synthesis, and the hash must be computed from the post-guard state so a pinned hash is always the hash of the render that ships): the guard walks each stream's pieces (the stream utterances when present, else the whole-span text piece) from the LAST token backwards, and trims trailing tokens that positively contradict the absorbed mixture rows' render-token-walls payload — a different-voice payload word covering a majority of the token's derived interval at a wall width above a floor, normalizing differently under a frozen operator.
- Mid-row tokens are structurally never examined (the attested needle run sits mid-row and must survive); tokens at walls the payload does not cover (recovery zones) and same-badge differences stay; absorbed rows without a wall payload contribute no walls (per-row fail-open — the hash/fingerprint verdict remains the fail-closed layer).
- The guard edits the utterance piece texts AND re-derives each stream's `text` as the join of its pieces, so the hashed content and the rendered rows stay the same bytes (production synthesis always carries utterances; the splice renders from them while the hash reads `text`).
- **Census-first enactment:** the guard lands in the production repair path in log-only census mode for every route (legacy phrase-loop included — that route has no attestation gate and would otherwise ship trims ungated at merge). Trim enactment on any route is a follow-up flip unlocked only after the env-gated offline census (counts-only) meets its acceptance bar on the reference meeting AND a second meeting, with the user's dated review — mirroring the live "Candidate census precedes render changes" pattern. Until enactment, no render changes at merge: the machinery stays render-inert.
- Census/evidence discipline: a trimmed-or-would-trim window records token-only evidence (`tail-trimmed:<n>` beside the outcome token for talkover seeds whose splice applied; a token-only log line for legacy windows); every guard log line is token-only (indices, walls, counts — never the trimmed token, never the covering mixture word, never any row text).

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `speaker-diarization`: two requirement-level changes — (1) a new requirement: rebuilt-window tails carry mixture-payload provenance (guard semantics, census-first enactment gate, hash-truth binding); (2) the "Talk-over repair promotion is decode-hardened and ear-gated" requirement is amended: the phrase-loop scenario and census enumeration accommodate the guard (census note beside the outcome token; phrase-loop splice-through-guard once enacted).

## Impact

- `frontend/src-tauri/src/audio/speaker/run_assembly.rs` — the guard lives as a public function beside the splice it feeds (`synth_rows_for_streams` / `repair_overlap_rows_checked` / `repair_render_sha256` all in this module, which also houses `vetted_stream_pair`); invoked in `commands.rs` between `order_repairs_for_apply` and `repairs_admissible_for_splice`. Red tests first, synthetic fixtures only; red-test fixtures use populated-utterance syntheses (the production shape) so a text-only trim cannot pass silently.
- `frontend/src-tauri/tests/` — the repair probe and both harness suites gain explicit guard-pre-pass invocation (today they call the splice directly and would never exercise a commands.rs-level pre-pass); the attestation-gate suite gains census-evidence cases. Render-text needles keep reading verbatim from the local evidence fixture at runtime, never code.
- No API, DB, store-schema, or model changes. The ConvTasNet 2-stream root cause is deliberately out of scope (needs a new model export + hosting decisions — tracked in fork issue #7 sketch 2).
- Certification process: unchanged until enactment — no store entry is written, no certification listen happens in this change. A post-enactment attestation of a guard-affected window pins the post-guard render hash and re-certifies by ear.
