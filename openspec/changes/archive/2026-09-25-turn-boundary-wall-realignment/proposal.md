# Proposal: turn-boundary-wall-realignment

## Why

Ear ruling S7c (invariant): "I don't know." (meeting cde5c264) is UserA's.
The user's ear replay on the 34.5–37.7 s clip (2026-09-25) located the words at
**36.1–36.7** — but whisper's token walls pin the atom at 35.51–36.51,
straddling the engine's voice-change turn boundary at 36.08. The models were
never wrong about the voice on that audio: pyannote puts UserA's local voice
at 0.43–0.9 mass exactly at 36.1–36.7, and UserB's fading tail (0.68→0.05)
on the smeared span. The defect is whisper's cross-attention wall placement:
DTW smears word walls backwards across a silence trough (35.9–36.1) onto the
previous speaker's trailing audio. The earlier buried-voice/overlap hypothesis
is disproven — the user attests no simultaneous speech, and pyannote's overlap
mass at the atom is only 0.04–0.05. (The overlap-separation-prepass change is
parked on this evidence.)

Class: an engine turn is voice-homogeneous and a sentence is one voice (ear
law), so a sentence atom straddling a genuine voice-change boundary is by
construction a wall error. Today the render assigns such atoms by wall-span
majority — silently placing the words in the wrong speaker's row.

## What Changes

- One pure realignment pass over wall atoms before assignment: an atom that
  straddles a non-sustained boundary between two different-voice turns, whose
  sentence-final punctuation wall sits on the far side, and whose far side has
  unclaimed voiced head room, is re-anchored into that head room (scaled to
  fit between the boundary and the next atom's start).
- Guards keep it conservative: side-past-boundary ≥ 300 ms, head room ≥
  300 ms, scale factor within [0.5, ∞) with room ≥ half the atom duration,
  boundary tolerance 250 ms, one atom per boundary, real-span units only.
- S7c graduates from amended known-limitation to a HARD gate pin.

## Impact

- Affected: `alignment.rs` (one function + call site + tests), `ear_truth_gate.rs`
  (S7c pin window update + waiver removal), ear-truth fixture (S7c span →
  36.0–36.8, known_limitations cleared — done), parked separation change note.
- Out of scope: whisper re-decoding, any new model, any hardcoded span/label.
- Risk: other straddling atoms meeting-wide — guarded by the full pin suite,
  fracture scan, and duplicate scan on every gate run.
