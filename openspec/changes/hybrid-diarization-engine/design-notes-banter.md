# Banter-zone analysis — user adjudication follow-up (2026-09-05)

User adjudication: **ear wins on S7** ("32.0–38.0s is one voice — unequivocal").

## Voice-affinity probe (tests/banter_voice_probe.rs, TitaNet all-pairs cosine)

| span | ear identity | u-5yr | u-yeah | cynd13 | disp34 | disp41 | ric |
|---|---|---|---|---|---|---|---|
| 9.38–13.03 five-years | (see note) | — | .228 | .217 | .245 | .084 | .112 |
| 29.50–32.16 yeah-sure | USER | .228 | — | .630 | .574 | .551 | .152 |
| 13.42–14.78 yeah-that | (see note) | .217 | .630 | — | .442 | .446 | .183 |
| 34.66–38.64 is-Ricardo | USER (S7 verdict) | .245 | .574 | .442 | — | .507 | .260 |
| 41.98–52.85 ping-in | USER (S7 verdict) | .084 | .551 | .446 | .507 | — | .127 |
| 2802.08–2820.13 | RICARDO | .112 | .152 | .183 | .260 | .127 | — |
| 39.00–39.93 okay | Cynthia | .370 | .085 | .104 | .109 | .006 | .025 |

Reading: the disputed "is Ricardo? / I don't know. Let me ping in" stretches
match the user's 29.5+ voice at 0.55–0.57 and sit 0.11–0.26 from Ricardo's
stretch — **TitaNet agrees with the ear; the machine's 34.66s boundary was a
CLUSTERING artifact** (greedy centroids stale after formation). Fixed by the
bounded Lloyd refinement loop (`refine_loop`): S7 passes, boundary gone.

Note: the probe also suggests the "aged like five years" sentence (9.38–13.03)
is Cynthia's and "Yeah. That's right. Oh, man" (13.42) is the user's — the
reverse of the session's working assumption. The fixture pins are
identity-free so no entry changes; recorded here for the enrollment follow-up.

## Remaining gate failures (S4/S5/S6) — one root cause

All three live in 26.4–34.66s, where EVERY pyannote window decodes one
continuous speaker across the real Cynthia→user→Cynthia exchange. There is no
corroborated boundary signal for the engine to find, and the mixed piece
(27.34–32.16, embedding margin 0.030) cannot be split locally without
destabilizing correct clusters globally:

- scan attempt 1 (unrestricted): 7/14 — regressed S2/S3/S8/S9/S10/S14.
- scan attempt 2 (transition-restricted): 6/14 — same collateral.
- scan attempts reverted; Lloyd-only state retained: **11/14, 0 invariant
  violations**, failures S4/S5/S6 (this root cause).

Scan attempt 3 (own-side evidence on either side of the run + transition
restriction + Lloyd): 6/14 — identical collateral. Root finding: the turn
stream is a sequential chain; any locally injected split propagates boundary
shifts forward to the next stable anchor. Three measured attempts confirm the
fix class does not exist within the pinned signal set.

**Enrollment lever BUILT (2026-09-05, commit pending):** enrolled references
(named-speaker fingerprints from the stamped pool) enter clustering as extra
stable centroids — the piece-cluster budget shrinks by the reference count,
and the Lloyd loop resolves ambiguous pieces/blips against them. Inert while
the pool is empty (verified: gate unchanged at 11/14 with 0 references).
Activation: user renames this meeting's three Speaker badges → the rename
flow relinks cluster embeddings to the named speakers → pool seeds → gate
re-runs with references → S4/S5/S6 verify against known voices.

Paths to close S4/S5/S6 (user decision at review):
1. KNOWN-LIMITATION sign-off (models lack the signal; boundary invented from
   text would contradict the success-path design).
2. Speaker enrollment (the planned follow-on change): a user reference voice
   would both identify the mixed piece's halves and pull them to the correct
   clusters at verification time.
