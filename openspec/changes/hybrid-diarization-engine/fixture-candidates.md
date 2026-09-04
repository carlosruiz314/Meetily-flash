# SUPERSEDED — see fixture-answers.md (user confirmed/denied 2026-09-04; several hypotheses corrected there).

# Fixture candidates — task 1.1 (user confirm/deny)

Each entry becomes `ear_truth_cde5c264.json` data `{start_s, end_s, kind, params}`.
Confirmed entries only enter the fixture; two confirmed entries will be designated
hold-out (never used for calibration). Sources: live probes (`pyannote_activity_diag.rs`),
sim runs, the user's ear-verified statements.

## Seeds (user-ear-established; re-confirm to enter the fixture)

**#1 `single_voice` 9.38–13.03** — "...you've aged like five years." One voice
(the sentence you verified); no boundary inside, none at the whisper-row edge 12.07.

**#2 `voice_change_at` within 159.3–170, change at ≈163 ±1; earlier-side text tail
"And I was like, oh, when you put a that one"** — your 02:12–02:50 example: one
voice change in the stretch, the tail sentence on the earlier speaker.

**#3 `distinct_speaker` 2814–2824** — Ricardo's 46:58 interjection is a different
voice from the surrounding run (both neighbors).

## Mined candidates (confirm or deny each)

**#4 `voice_change_at` ≈13.0–13.4** — "…five years." (ends) → "Yeah. That's right.
Oh, man" (starts). Current pipeline keeps both in one Speaker-1 turn; pyannote's
track flips at 13.03. Hypothesis: a voice change the pipeline missed.

**#5 `single_voice` 25.3–30.0 (NO change at 26.11)** — pipeline flips Speaker 1 →
Speaker 0 mid-sentence at 26.11 ("...do we wan[t] | to, let's, wait, we've got to
record this"). Pyannote held one speaker across it. Hypothesis: same voice, the
flip is fabrication.

**#6 `voice_change_at` ≈30.0** — "...we've got to record this" (ends) → "Yeah,
sure, sure, sure. Yeah, for Paulina, right?" (starts). Both the pipeline and
pyannote flip here. Hypothesis: a real turn change.

**#7 `single_voice` 16.1–20.3** — "Okay. I have some updates. Cool. On the" — a
clean single-voice run (pins a long span of one speaker mid-banter).

**#8 `single_voice` 2803–2820** — "I can still analyze all the marketing
requests. I was like mocking Flavia..." — one voice across the whole stretch
(pipeline labels it Speaker 2).

**#9 `voice_change_at` ≈2774.5 (46:14)** — "probably something that we" → "can
take. Can you come again?" — candidate cross-speaker sentence continuation in a
crosstalk-flagged zone. Note: this one genuinely looks like two voices trading
halves of a sentence; if your ear says it's one voice, the engine has a real
defect at this spot.

**#10 `single_voice` 30.0–52.8 (tail)** — "...Where is Ricardo I don't know. Let
me ping in. I can't. I can't. I saw you there in the meeting room alone..." —
the sim merged this whole stretch as one speaker after the textless-run fix.
Ear question: is "Where is Ricardo" the same voice as "I don't know. Let me ping
in"?

## Proposed hold-outs (pick 2 of the confirmed)

Recommend **#3** and **#7** — farthest from the calibration-critical splits.
