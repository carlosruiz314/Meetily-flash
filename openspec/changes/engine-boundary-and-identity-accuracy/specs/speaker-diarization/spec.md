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
(references included). References SHALL be one mean-normalized voiceprint
per named speaker, selected in deterministic speaker-id order
(`list_enrollment_refs`): per-row, unordered refs would let one voice's
extra enrolled embeddings crowd other voices out of the anchor budget and
make badge numbering unstable across runs.

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

## ADDED Requirement: Proportional sentence placement honors engine boundaries

When a transcript row's words carry proportional (token-less) spans, its
sentences SHALL be assigned by the engine's turn boundaries: each interior
voice change SHALL fall on a sentence gap — never inside a sentence — by
snapping boundaries to the nearest sentence gap in reading order, and
sentences between two snapped boundaries SHALL share the voice owning their
stretch. Where a voice change crosses a pyannote-silent gap, the sentence
whose wall-clock share ends at the incoming voice's turn start SHALL join
the incoming voice, and — within a boundary-anchored unit — a sentence
whose span reaches a rescue-attributed seam SHALL take the seam's voice.
Token-aligned rows keep per-sentence
overlap majority (their word times are exact).

#### Scenario: A back-channel drifting left of its true boundary still renders under its own voice

- **GIVEN** a 26.84s token-less source row holding 11 sentences across six
  voice changes, where proportional placement puts the "Oh, man." atom
  [14.42,15.59) although UserB's voice starts at 15.64 (pyannote-silent
  gap, rescue-attributed)
- **WHEN** the render assigns the row's sentences to engine turns
- **THEN** "Oh, man." renders under UserB's badge, not the preceding
  speaker's
- **AND** the row's other sentences keep their ear-attested badges
- **AND** no sentence renders under a badge that changes mid-sentence
