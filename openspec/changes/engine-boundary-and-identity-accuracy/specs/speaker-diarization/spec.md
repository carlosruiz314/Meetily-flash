# Delta: speaker-diarization (engine boundary and identity accuracy)

## AMENDED Requirement: Speaker turns derive from pyannote speech runs with verified sub-run voice-change splits

The success-path engine SHALL derive turn boundaries from one pyannote
segmentation pass over per-frame probability masses, with sub-run splits
from TWO evidence sources:

1. Window-corroborated label/slot changes (existing trust-zone mechanism,
   unchanged).
2. **Embedding voice-flip checks at confusion valleys** (new): run frames
   where the argmax speaker mass drops below 0.7 while a second speaker's
   mass reaches 0.15 mark candidate valleys; the engine SHALL embed fixed
   windows either side of a valley start and split the piece ONLY when
   both sides embed decisively (margin ≥ the ambiguity margin) to
   DIFFERENT final centroids. Undecided or same-voice valleys SHALL NOT
   split. The scan SHALL be bounded (≤3 splice rounds, one split per piece
   per round) and deterministic.

Rationale: pyannote holds the argmax through short other-voice
back-channels (measured: the user's 12.0s "Yeah" decodes to contested mush
with the argmax held — no candidate exists for smoothing to recover), so
boundary recall requires an embedding-level check where the decode is
confused. The whole-atom text layer makes the added boundaries safe: they
never fracture a sentence, only move whole atoms between badges.

#### Scenario: A back-channel the segmentation model cannot decode still gets its boundary

- **GIVEN** a held speech run whose decode collapses to contested masses
  without a slot change (the S2 signature at 12.0s)
- **WHEN** the engine runs and the embedding check finds both sides
  decisively matched to different centroids
- **THEN** the run splits at the valley start and the resulting turns
  carry the two voices
- **AND** the ear-truth gate asserts the voice change at the pinned second
  as an ASSERTED entry (no waiver)

#### Scenario: Confusion without identity difference never splits

- **GIVEN** a contested valley where both side-windows resolve to the same
  centroid or either margin is below the ambiguity margin
- **WHEN** the engine evaluates the valley
- **THEN** no split is inserted

## AMENDED Requirement: Piece clustering is order-independent and enrollment-anchored

On the success path, labeled pieces SHALL be clustered by average-linkage
agglomerative clustering against the configured merge threshold (merge
while the closest pair's average linkage ≥ threshold; ties by smallest
index pair), replacing the greedy online pass. The most-isolated
merge-to-cap, nearest-centroid refine loop, and phantom-centroid pruning
are unchanged. Clustering remains deterministic, bounded by the piece
shed-to-cap, and off the async executor.

Named-speaker enrollment embeddings (the stamped pool) SHALL enter
clustering as stable extra centroid anchors — at most `max_speakers - 1`
references, leaving at least one cluster slot for unknown voices — and the
refine loop SHALL resolve ambiguous pieces against the full centroid set
(references included).

Rationale (measured 2026-09-09): the greedy pass seeded centroids in
processing order and converged to two centroids for a three-voice meeting —
the most prolific voice matched neither (0.12–0.32 anchor similarity) and
another speaker's speech rendered under his badge. AHC measures the
embedding matrix directly; enrollment anchors resolve voices whose short
pieces do not mutually cohere at the threshold (same-voice back-channel vs
long-stretch similarity ~0.28).

#### Scenario: A prolific voice with no self-coherent centroid still gets its own badge

- **GIVEN** a three-voice meeting where one voice's pieces do not cluster
  at the merge threshold, with that voice enrolled in the stamped pool
- **WHEN** the engine clusters and refines
- **THEN** the enrolled voice's pieces resolve to the enrollment anchor's
  cluster and the other voices keep distinct badges
- **AND** the ear-truth gate's single-voice and voice-change entries all
  pass asserted
