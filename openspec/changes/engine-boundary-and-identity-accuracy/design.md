# Design: engine boundary and identity accuracy

## Context

Both bars hold simultaneously: accurate speaker detection AND no split
sentences. The whole-atom text layer (no-split-sentences) makes extra
boundaries SAFE — a boundary can never fracture a sentence, only move an
atom between badges. Accuracy work therefore maximizes boundary RECALL and
identity PRECISION, both verified by the ear-truth gate.

Measured 2026-09-09 on cde5c264 (offline probes over the gate's frame-mass
cache; evidence in `openspec/exploration/bothbars-*.log` and
`identity-*.log`).

## D1: Sub-run voice-flip scan (boundaries pyannote cannot decode)

pyannote-segmentation-3.0 holds the argmax through short other-voice
back-channels: at the S2 pin (the user's "Yeah" at 12.0s inside the held
run [9.38,13.03]) the decode turns to contested mush (sp1 0.94→0.26, second
voice 0.24–0.33, silence up to 0.48) WITHOUT flipping — no slot change, so
no split candidate ever exists and no smoothing/corroboration knob can
recover the boundary. The miss class is structural.

Mechanism: `run_assembly::confusion_valleys` detects run frames where the
argmax speaker mass < 0.7 while a second speaker's mass ≥ 0.15 (exactly the
measured signature), merged into valley spans (gaps < 10 frames). The
engine, per labeled piece ≥ 2.0s, embeds 1.2s windows either side of each
valley start and splits ONLY when both sides embed decisively (margin ≥
AMBIGUITY_MARGIN) to DIFFERENT final centroids — the same identity bar an
ordinary labeled piece must clear. Bounded rounds (3), one split per piece
per round, deterministic splice. Cost: 2 embeddings per qualifying valley
(639 valleys meeting-wide; geometry constraints gate most out).

## D2: Average-linkage agglomerative clustering (identity, order-free)

The greedy online clustering seeds centroids in processing order and
drifts: on cde5c264 it converged to TWO centroids — UserB (0.72 anchor
similarity) and UserC (0.71) — while USERA, the most prolific voice,
matched neither (0.12–0.32). His pieces flipped between clusters on thin
margins (with only two centroids, best-minus-second margins are large even
when the best cosine is poor) and UserC's real speech shared UserA'
badge (2801–2821 rendered as Speaker 0).

`cluster_pieces_ahc` replaces the greedy pass on the success path:
clusters form from members' MUTUAL similarity, independent of order; the
merge threshold keeps its meaning (merge while the closest pair's average
linkage ≥ threshold); merge-to-cap, refine loop, and phantom pruning are
unchanged. Deterministic (ties → smallest index pair).

## D3: Enrollment references are the identity anchor

AHC alone still cannot resolve UserA: his pieces do not mutually cohere at
the merge threshold (a 0.8s back-channel embeds only ~0.28 similar to his
own long stretches). This is the designed enrollment lever, now
load-bearing: named-speaker fingerprint embeddings enter clustering as
stable extra centroids (`take(max_speakers - 1)` references; leftover
meeting-internal clusters carry unknown voices), and ambiguous pieces
resolve against known voices during the refine loop.

Measured with two ear-attested seeds (UserA: the S2 "Yeah" 12.0–12.8s +
pre-join 42.2–50.0s; UserB: S1 9.75–11.8s + 35.0–36.0s): all three
voices resolve — centroids 0=UserA (0.35–0.52 anchor similarity),
1=UserB (0.72), 2=UserC (0.76). UserC's stretch renders under his
own badge; UserA' rows are consistent. The engine takes references from
the stamped pool (named speakers only, one mean-normalized voiceprint per
speaker in deterministic speaker_id order via `list_enrollment_refs`) — in
production these come from the rename/enrollment flow; gate + persist runs
read the same pool, so test and production agree. Consequence, accepted by
design: the gate's identity resolution is pinned only as firmly as the
enrollment pool state at run time — a green identity run is a statement
about that pool, and a later wipe/regression of the pool changes what the
gate anchors against (this is exactly how the 2026-09-14 re-diarization
regressed to two badges until the UserA fingerprint was restored).

## D4: Offline iteration infrastructure (how this is developed)

Engine iterations do NOT re-pay model inference: the gate's provenance-
checked frame-mass cache (`gate_frame_masses.json`) replaces the ~16-min
pyannote pass, a raw f32 samples cache replaces the ~6.5-min decode, and
pure-boundary probes need no model at all
(`tests/engine_offline_probe.rs`). The ear-truth gate remains the only
full-cost authority: 16/16 asserted entries, 0 cross-badge fractures, no
waivers.

## D5: The render honors the engine's boundaries (ear verdict 2026-09-20)

The user failed the persisted output on `[12.67] Speaker 0: "Yeah, that's
right. Oh, man."` — "Oh, man." is UserB's. Root cause chain: the source
row is one 26.84s block holding 11 sentences across six voice changes;
token-less proportional placement drifts ~1s by mid-row, so the "Oh, man."
atom landed [14.42,15.59) — fully left of the true 15.64 UserB onset
(pyannote decodes that phrase as silence; the gap rescue attributed
[15.64,15.86] to her at margin 0.057) — and per-atom overlap majority then
overrode the engine's boundary. The engine was right; the render re-decided
with worse information.

Fix (alignment.rs, proportional rows only — token-aligned rows keep exact
per-atom majority): boundary-anchored assignment. The ear decree — a voice
never changes mid-sentence — makes boundaries fall BETWEEN atoms by
construction: snap every interior turn boundary to its nearest atom gap
(monotone, one gap per boundary) and assign atoms from boundary-delimited
stretch(es), whose voice is constant; stretch-majority dilutes the skew
that broke single atoms. Silent-seam absorption: when a boundary's flanking
turns leave a pyannote-silent gap, the atom whose wall END hugs the incoming
voice's turn start joins the incoming voice (the seam's true text sits at
the seam; proportional drift put it left). Engine plumbing: accepted rescue
sub-windows ride `EngineOutput.rescue_seams` into the align, where atoms
intersecting a seam take the seam's voice — identity evidence beats
geometric skew whenever they conflict. Failure mode, accepted: a truly
outgoing-voice atom whose wall END hugs a silent seam flips; the gate's 16
ear pins and fracture scan adjudicate meeting-wide.

## Known residual (out of scope here)

Token-less proportional placement is bounded by wall-clock shares; atoms
whose true audio sits farther than the silent-seam rule reaches ("I don't
know." / "I can't." in the 32.51–40.24s row) can still take the neighboring
badge. Full fix needs word-level timestamps — tracked separately. The
silent-seam rule above shrinks the class to non-silent seams. Two softenings
keep the rule honest meanwhile: an atom's word-share (its absorption reach)
is inflated when the unit's word range bridges two rejoined source rows
(the inter-row wall gap rides along), and the rule disengages entirely for
units where any stretch has no diarization overlap or boundaries don't snap
— those fall back to per-atom majority, where the rescue-seam pin does not
apply either.
