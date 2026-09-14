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
drifts: on cde5c264 it converged to TWO centroids — Cynthia (0.72 anchor
similarity) and Ricardo (0.71) — while CARLOS, the most prolific voice,
matched neither (0.12–0.32). His pieces flipped between clusters on thin
margins (with only two centroids, best-minus-second margins are large even
when the best cosine is poor) and Ricardo's real speech shared Carlos'
badge (2801–2821 rendered as Speaker 0).

`cluster_pieces_ahc` replaces the greedy pass on the success path:
clusters form from members' MUTUAL similarity, independent of order; the
merge threshold keeps its meaning (merge while the closest pair's average
linkage ≥ threshold); merge-to-cap, refine loop, and phantom pruning are
unchanged. Deterministic (ties → smallest index pair).

## D3: Enrollment references are the identity anchor

AHC alone still cannot resolve Carlos: his pieces do not mutually cohere at
the merge threshold (a 0.8s back-channel embeds only ~0.28 similar to his
own long stretches). This is the designed enrollment lever, now
load-bearing: named-speaker fingerprint embeddings enter clustering as
stable extra centroids (`take(max_speakers - 1)` references; leftover
meeting-internal clusters carry unknown voices), and ambiguous pieces
resolve against known voices during the refine loop.

Measured with two ear-attested seeds (Carlos: the S2 "Yeah" 12.0–12.8s +
pre-join 42.2–50.0s; Cynthia: S1 9.75–11.8s + 35.0–36.0s): all three
voices resolve — centroids 0=Carlos (0.35–0.52 anchor similarity),
1=Cynthia (0.72), 2=Ricardo (0.76). Ricardo's stretch renders under his
own badge; Carlos' rows are consistent. The engine takes references from
the stamped pool (named speakers only) — in production these come from the
rename/enrollment flow; gate + persist runs read the same pool, so test
and production agree.

## D4: Offline iteration infrastructure (how this is developed)

Engine iterations do NOT re-pay model inference: the gate's provenance-
checked frame-mass cache (`gate_frame_masses.json`) replaces the ~16-min
pyannote pass, a raw f32 samples cache replaces the ~6.5-min decode, and
pure-boundary probes need no model at all
(`tests/engine_offline_probe.rs`). The ear-truth gate remains the only
full-cost authority: 16/16 asserted entries, 0 cross-badge fractures, no
waivers.

## Known residual (out of scope here)

Token-less source rows assign word times proportionally, so atoms
straddling a true boundary ("I don't know." / "I can't." in the
32.51–40.24s row) can take the neighboring badge. Fix needs word-level
timestamps or skew-tolerant atom assignment — tracked separately.
