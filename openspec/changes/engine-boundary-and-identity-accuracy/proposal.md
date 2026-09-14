# Proposal: engine boundary and identity accuracy

## Why

Both bars must hold at once: accurate speaker detection AND no split
sentences. The whole-atom text layer (no-split-sentences) already makes
extra boundaries safe — a boundary can only move a whole atom between
badges, never fracture a sentence. The accuracy gap is therefore two
distinct engine defects, both measured on cde5c264 on 2026-09-09:

1. **Boundary recall**: pyannote-segmentation never decodes short
   other-voice back-channels — at the user's 12.0s "Yeah" (ear pin S2, the
   gate's last standing known-limitation, user ear clip_D 2026-09-07:
   "definitely around 0.5s not 1.9s") the decode turns to contested mush
   while holding the argmax, so no split candidate ever exists and no
   smoothing knob can recover the boundary.
2. **Identity precision**: the greedy online clustering converged to two
   centroids for a three-voice meeting. Carlos — the most prolific voice —
   matched neither (anchor similarity 0.12–0.32), his pieces flipped
   between clusters on thin margins, and Ricardo's real speech rendered
   under Carlos' badge.

## What changes

- **Sub-run voice-flip scan**: at pyannote confusion valleys (argmax < 0.7
  with a second voice ≥ 0.15), embed windows either side of the valley
  start and split only when both sides decisively match DIFFERENT final
  centroids. Bounded, deterministic.
- **Average-linkage agglomerative clustering** (`cluster_pieces_ahc`)
  replaces the greedy pass on the success path: order-independent
  cluster formation, same threshold semantics, merge-to-cap/refine/prune
  unchanged.
- **Enrollment anchors are load-bearing**: named-speaker fingerprints from
  the stamped pool enter clustering as stable extra centroids (≤
  max_speakers − 1 references); ambiguous pieces resolve against them.
  Seeded for this meeting from ear-attested windows (reversible
  `emb-earseed-*` rows).
- **Fixture**: S2 promoted from known-limitation to a fully asserted gate
  entry (waiver removed).
- **Iteration infrastructure**: provenance-checked frame-mass cache, f32
  samples cache, and no-model boundary probes — engine iterations run
  offline in seconds-to-minutes; the ear-truth gate remains the only
  full-cost authority.

## Capabilities

### <speaker-diarization>
- Difference: two boundary evidence sources (window corroboration +
  embedding flip checks at confusion valleys); order-independent
  agglomerative clustering; enrollment-anchored identity resolution.
- Impact: S2 passes asserted; all 16 ear entries pass with zero waivers;
  0 cross-badge fractures meeting-wide; three voices resolve with
  distinct badges (Ricardo no longer conflated with Carlos).

### <no-split-sentences>
- Difference: none to the text layer's contract — the decree (a voice
  does not change mid-sentence; every cross-badge fracture fails hard)
  is preserved and now documented as the governing constraint that makes
  boundary-recall work safe.
