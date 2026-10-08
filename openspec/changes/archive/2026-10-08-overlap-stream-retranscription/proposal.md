# Proposal: overlap-stream-retranscription

## Why

The user attested the real crosstalk at 1055.5–1057.5 s (fixture S16): the
user's phrase and userC's two-word interjection (verbatim text lives only
in the local fixture) collide in one window. The render today shows ONE
badge for the whole window and whisper's mixture transcription interleaves
the voices out of order, with an echo copy — both bars the user set
(accurate attribution AND no split/scrambled sentences) fail there. The
separation pre-pass (archived 2026-09-28) proved the voices are
recoverable: the stop-gate measured both streams decisively identified
(margins 0.135 / 0.147) at that exact span. What it deliberately did NOT do
is change the render — one atom carries one badge, and the text scramble
lives in whisper's mixture transcription.

## What Changes

Re-transcribe the separated streams over both-streams-decisive overlap
spans and render per-voice rows:

- For each overlap span where BOTH streams are decisively identified
  (existing margin bar, DIFFERENT clusters), run the meeting's Whisper
  model on each stream under a deterministic pinned-language profile and
  synthesize one row per stream: per-voice text at the span walls, badged
  by the stream's ref-anchored speaker.
- The mixture rows inside the span are replaced by the stream rows in the
  RENDER only. `transcript_sources` stays untouched (immutable-source
  invariant) — stream rows are render-level artifacts.
- A missing separation model, an undecisive stream, a same-badge collapse,
  or a garbage stream (silence / hallucinated text) degrades to today's
  render, byte-identical (comparator pinned by test).
- No hardcoded overrides anywhere: badges change only through the existing
  evidence channels (ref-anchored margin-gated votes); text changes only
  through the separated audio itself. Ear rulings tune the general
  thresholds, never individual spans.

## User-visible outcome

The 1055–1058 window renders as two rows — the user's sentence in order
under their badge, userC's interjection under theirs — instead of one
scrambled mixture row. Acceptance is the user's ear on the calibration
clip set (up to ~20 moments: word-loss-flagged first, remainder evenly
spaced, drawn from the gate's census — which also records every
trigger-fired span the detector rejected, so over-firing is visible and
the dials are tunable from evidence), preceded by the agent's own read of
the full rerun transcript vs the original.

## Capacities touched

speaker-diarization (render synthesis); no new capability.
