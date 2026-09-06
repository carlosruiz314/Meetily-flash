# Review — hybrid-diarization-engine (task 5.4)

Gate evidence: `20260906-113421.log` — GATE SUMMARY: 11 passed, 0 known-limitation, 3 FAILED of 14 entries; failed: ["S4_cynthia_banter", "S5_cynthia_to_user_yeah_sure", "S6_user_to_cynthia_yeah"]
Hard invariant: GATE: invariant scan: 0 violation(s) over 239 turns

Text quality inside turns is whisper's output and out of scope —
this review judges ATTRIBUTION only.

## Fixture verdicts

| entry | kind | span | verdict |
|---|---|---|---|
| S14_banter_anchor_161 | voice_change_at | 155.0–170.0s | **PASS** |
| S1_five_years_sentence | single_voice | 9.4–12.8s | **PASS** |
| S2_user_to_cynthia_13s | voice_change_at | 12.0–15.5s | **PASS** |
| S3_updates_run (hold-out) | single_voice | 15.5–20.8s | **PASS** |
| S4_cynthia_banter | single_voice | 24.5–29.5s | **FAIL** |
| S5_cynthia_to_user_yeah_sure | voice_change_at | 28.5–31.0s | **FAIL** |
| S6_user_to_cynthia_yeah | voice_change_at | 30.5–33.0s | **FAIL** |
| S7_where_is_ricardo_head | single_voice | 32.0–38.0s | **PASS** |
| S8_okay_interjection_start | voice_change_at | 37.5–39.2s | **PASS** |
| S9_okay_interjection_end | voice_change_at | 39.3–43.0s | **PASS** |
| S10_can_you_come_again_trade | multi_voice | 2772.0–2777.5s | **PASS** |
| S11_ricardo_stretch | single_voice | 2803.0–2820.0s | **PASS** |
| S12_cynthia_to_ricardo | voice_change_at | 2801.5–2804.5s | **PASS** |
| S13_ricardo_to_cynthia (hold-out) | voice_change_at | 2812.0–2826.0s | **PASS** |

## Pinned regions — old pipeline vs new engine

### 0–45s

**OLD pipeline (persisted turns):**

- `   5.87–  12.07` Speaker 0: How's it going? All good, all good. You? I don't like you've aged like
- `  12.07–  15.84` Speaker 1: five years. Yeah. That's right. Oh, man
- `  16.20–  20.18` Speaker 0: Okay. I have some updates. Cool. On the
- `  20.18–  26.11` Speaker 1: roadmap, hopefully. Okay. Let's go. So for search, do we want
- `  26.11–  29.78` Speaker 0: to, let's, wait, we've got to record this
- `  30.09–  34.92` Speaker 1: Yeah, sure, sure, sure. Yeah, for Paul ina, right? Where
- `  34.92–  39.27` Speaker 0: is Ricardo
- `  39.27–  55.18` Speaker 1: I don't know. Let me ping in. I can't. I can't. I saw you there in the meeting room alone 

**NEW engine (turns):**

- `   1.30–   8.57` sp0
- `   9.38–  13.03` sp1
- `  13.42–  14.78` sp0 lowconf
- `  16.12–  20.30` sp1 lowconf
- `  20.81–  24.49` sp0 lowconf
- `  25.31–  26.43` sp1 lowconf
- `  27.34–  38.64` sp0
- `  39.00–  39.93` sp1 lowconf
- `  41.98–  55.01` sp0 lowconf

Entries here: **S1_five_years_sentence = PASS**, **S2_user_to_cynthia_13s = PASS**, **S3_updates_run = PASS**, **S4_cynthia_banter = FAIL**, **S5_cynthia_to_user_yeah_sure = FAIL**, **S6_user_to_cynthia_yeah = FAIL**, **S7_where_is_ricardo_head = PASS**, **S8_okay_interjection_start = PASS**, **S9_okay_interjection_end = PASS**

### 130–180s

**OLD pipeline (persisted turns):**

- ` 128.41– 132.11` Speaker 1: for motors. Fine. Okay, we're going to do that. it's a it's a given
- ` 132.67– 160.06` Speaker 0: yeah i think it's it's two spr ints it's like a third of your capacity because you have i 
- ` 160.06– 172.12` Speaker 1: was like, oh, when you put a that one, it's a lot of capacity, actually. It is quite a lot
- ` 172.12– 174.12` Speaker 0: it. Well, you need that for
- ` 174.40– 181.76` Speaker 1: hybrid search anyway, right? Yeah, but that's not going to... Like, motors is a completely

**NEW engine (turns):**

- ` 129.16– 132.00` sp0
- ` 132.64– 145.90` sp1
- ` 146.63– 147.69` sp0 lowconf
- ` 148.09– 162.34` sp1 lowconf
- ` 162.78– 170.89` sp0 lowconf
- ` 171.79– 173.81` sp1
- ` 174.44– 180.41` sp0

Entries here: **S14_banter_anchor_161 = PASS**

### 2770–2830s

**OLD pipeline (persisted turns):**

- `2750.39–2777.94` Speaker 0: ne more thing to throw a wrench in all your plans. We have to take up this request from ma
- `2774.03–2776.14` Speaker 2: probably something that we
- `2776.36–2790.36` Speaker 0: can take. Can you come again? What's that? So there's a request from marketing. Let me jus
- `2792.99–2794.48` Speaker 2: Motors landing
- `2794.48–2803.00` Speaker 0: pages? It is a landing page thing, but I don't know where this...
- `2803.00–2820.06` Speaker 2: I can still analyze all the marketing requests. I was like mocking Fl avia a couple of mon
- `2820.60–2858.54` Speaker 0: right. You know, our boss, my boss is often right. That is a lesson I have learned. Yeah. 

**NEW engine (turns):**

- `2750.36–2772.53` sp1
- `2773.96–2775.41` sp2 lowconf
- `2776.29–2790.28` sp1
- `2792.95–2793.99` sp2 lowconf
- `2795.38–2802.08` sp1
- `2802.08–2820.13` sp2
- `2821.64–2856.03` sp1

Entries here: **S10_can_you_come_again_trade = PASS**, **S11_ricardo_stretch = PASS**, **S12_cynthia_to_ricardo = PASS**, **S13_ricardo_to_cynthia = PASS**

## Open entries (user chose to leave them open, 2026-09-05)

- **S4_cynthia_banter** — Clip 05: Cynthia throughout (user's 'yeah sure' starts at the very end). The pipeline's 26.11 flip ('do we wan|to') is fabricated.
- **S5_cynthia_to_user_yeah_sure** — Clips 05+06: Cynthia → user ('Yeah, sure, sure, sure. Yeah, for Paulina, right?' is the USER). User's 0:01 = 29.5 coarse; refined to the measured switch 30.0 (pyannote P2→P1), consistent with the -0.5..-1.1s coarse-reporting offset.
- **S6_user_to_cynthia_yeah** — Clip 06: Cynthia interjects 'Yeah' — inside the legacy pipeline's 30.09-34.92 row (missed change). User's 0:03 = 31.5 coarse; refined to the measured different-voice run 32.65-32.97.

Root cause: pyannote decodes 26.4–34.66s as one continuous speaker in
every window; no boundary signal exists. Enrollment with independent
reference audio is the path that can revisit these.
