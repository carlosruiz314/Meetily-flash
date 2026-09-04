# Fixture answers — task 1.1 (user ear-truth, recorded 2026-09-04)

User listened to the 10 clips in `fixture_clips/` once and answered per clip.
Answers are verbatim; absolute times resolved via recovered cut offsets
(`fixture-offsets.json`, cross-correlation ≥0.996 on 9/10 clips).
Voices: **user** (softer male), **Cynthia** (louder female), **Ricardo**.

## Verbatim answers

- `01` (8.00–14.53) — "no, it's two voices, mine (softer, male) and Cynthia's (louder, female)"
- `02` (155.00–170.02) — "yes, changes at 0:06"
- `03` (2812.00–2826.04) — "same person until 0:09 (Ricardo), then Cynthia afterwards"
- `04` (unlocatable) — "'five years' isn't audible in this clip. I think it got truncated right before it started"
- `05` (24.50–30.52) — "no, there's a 'yeah sure' from me at the end (0:05)"
- `06` (28.50–33.04) — "yes, switches from Cynthia to me at 0:01 where I say 'Yeah sure sure sure, for Paulina right?' and switches again to Cynthia at 0:03 where she answers 'Yeah'"
- `07` (15.50–20.83) — "yes"
- `08` (2802.00–2820.02) — "no, there's a bit of Cynthia at the beginning (0:00-0:01)"
- `09` (2772.00–2777.54) — "two people trading"
- `10` (32.00–41.02) — "mostly, but there's an 'okay' interjection from Cynthia on 0:06-0:07"

## Clip 04 disposition

Clip 04's audio matches no recording on disk (max correlation 0.11 across all
11 meeting files) — orphaned artifact of a bad cut. Its intended question
("does the voice change after 'five years'?") is answered by clip 01: the
user's sentence ends after "five years" (≈12.8) and Cynthia takes over
(≈13.0). Clip 04 is retired; not regenerated (no further user time).

## Corrections to the mined hypotheses (ear wins)

- #2 pin moves: change at **161.0 ±0.75** (was probe-estimated ≈163 ±1).
- #1 was NOT single-voice across the whole clip; the single-voice span is the
  user's sentence ≈5.9–12.8, Cynthia from ≈13.0 (old hypothesis #4 confirmed).
- #3 was modeled backwards: **Ricardo** holds 2812–2821, **Cynthia** from
  ≈2821 (not a brief ~2818 interjection). Clip 08 adds: Cynthia briefly at
  2802–2803, Ricardo 2803–2820.
- #5 confirmed (no change at 26.11) and extended: Cynthia holds 24.5–29.5;
  the "Yeah sure sure sure, for Paulina right?" sentence is the USER's, and
  Cynthia interjects "Yeah" at ≈31.5 — inside one pipeline row (missed change).
- #6 confirmed: real change Cynthia→user at ≈29.5 (both clips 05 and 06).
- #9 confirmed two voices trading; exact trade time not ear-pinned — pinned at
  the DB row edge 2776.4 with a wide tolerance.
- #10 confirmed single voice with Cynthia's brief "okay" at 38.0–39.0.

## Hold-outs

Designated per the recommendation (user raised no objection): entries
`S3_updates_run` (clip 07) and `S13_ricardo_to_cynthia` (clip 03) are
hold-outs — scored by the gate, never used for calibration decisions.
