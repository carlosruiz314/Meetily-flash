# Diarization TSE Spike — Decision Log

**Started:** 2026-07-16.
**Goal:** Test whether Target Speaker Extraction recovers the absorbed speaker ("UserB") on `meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323`.
**Constraint:** Pre-change exploration. Spike is throwaway Python; production integration (if any) is a separate OpenSpec change gated on the criteria below. Follow [[change-workflow]] once a proposal is warranted.

## Context recap
- Root cause CONFIRMED (STEP 7, 2026-07-06): absorption is embedding-level. Late-half UserB chunks sit at cos<0.3 to her centroid on native windows, measured BEFORE clustering (so no clustering fix can recover them).
- SIX methods disproven as fixes: embedding sweep (STEP 7/8), EEND-EDA (D5), SepFormer (D7), segmentation/threshold, F0 circular, F0 non-circular (this session).
- D8 correction: late half is effectively system-only (UserA barely spoke). Entanglement is UserB vs Speaker 2 WITHIN the system channel — mic/system channel separation does NOT fix it.
- F0 non-circular (this session): profile is finally correct (UserB 222 Hz / absorber 121 Hz on 88 clean early chunks), but reassignment failed — 0/22 reassigned chunks embed as UserB (19/22 at cos<0.3). F0 register ≠ speaker identity on this audio. Kept in repo as back-pocket auxiliary feature per user instruction; not a standalone fix.

## Hypothesis
TSE operates at the WAVEFORM layer, before embedding extraction. Enrolled with UserB's clean early reference, it may produce a coherent UserB-voiced stream from the late half even where the mixed-audio embeddings are contaminated. The enrollment cue — absent in blind SepFormer (D7) — is the mechanism by which TSE could succeed where blind separation failed.

## Pre-registered pass/fail criteria (no goalpost-moving)
- **PRIMARY:** on the TSE-extracted UserB stream, ≥50% of late-half voiced chunks embed at cos≥0.5 to her early centroid. (Currently ~0% in raw mixed audio.)
- **SECONDARY:** UserB's attributed late duration ≥ 400s. (Current: 40s.)
- **TIEBREAKER (resolves H1/H2):** near-silence output on late half → H2 (she went quiet); coherent voiced speech that embeds as UserB → H1 (absorption).
- **DOMAIN-MISMATCH OUTCOME (distinct, informative):** extracted chunks land at cos<0.3 to BOTH centroids → TSE failed the same way as SepFormer (D7), enrollment cue notwithstanding. This closes the separation-layer path.

## Checkpoint survey result (2026-07-16)
- **Feasible without training: YES.** Open TSE toolkits with pretrained models: `wesep` (wenet-e2e; same group behind the REAL-TSE challenge), SpeechBrain TSE recipes, VoiceFilter.
- **Domain-mismatch prior: PESSIMISTIC.** The REAL-TSE Challenge (IEEE SLT 2026) baselines — BSRNN trained on Libri2Mix — rank BOTTOM on real meeting data (TER 0.83 vs 0.61 top). Organizers call synthetic-trained TSE a "lower bound, not a ceiling." The REAL-T dataset and top-team models are NOT public (registration closed 2026-05-31). Only synthetic-trained checkpoints are obtainable.
- **Honest probability:** ~70-80% domain-mismatch failure; ~20-30% partial success (our late half is effectively 2-speaker + strong 88-chunk clean enrollment — more favorable than REAL-TSE's 6-speaker dinner/café scenarios).

## Why run it anyway
- Cheap (hours, not weeks).
- Decisive either way: success → production proposal; domain-mismatch failure → separation-layer path empirically closed (SepFormer + TSE both fail), leaving per-participant capture (Zoom SDK etc.) or accepting the limit as the only remaining options.
- Genuine minority chance our specific 2-speaker-late scenario beats the synthetic-training odds.

## Model + pipeline
- **Model:** `wesep` pretrained TSE (WSJ0-2mix-extr / Libri2Mix). Fallback: SpeechBrain TSE / VoiceFilter (spectrogram-mask, older but real-noise-trained — possibly more robust to codec artifacts).
- **Enrollment:** concatenate UserB's top clean early chunks (start<1800s, cos≥0.7 to centroid, voiced) → 15-30s reference WAV. Repeat for the absorber (cross-verification: extracting him should NOT recover UserB's late speech).
- **Audio source:** `audio.mp4` (109MB) at `C:\Users\UserA\Music\local-recordings\Meeting 2026-06-22_16-04-01_2026-06-22_14-04\`. Late half effectively system-only per D8. (Saved recording is mic+system mixed-mono; UserA quiet late so late half ≈ system.)
- **Measure:** nemo_titanet embeddings on extracted-stream native-window chunks; cosine to early centroids; compute PRIMARY/SECONDARY. Write extracted WAVs at ~49:00 and ~67:00 for user listen (ground-truth tiebreaker on H1/H2).

## Self-adversarial stress test (failure modes watched)
- **Domain mismatch (highest risk, literature-confirmed):** extracted chunks at cos<0.3 to both centroids → failure, same as D7. Pre-registered as a valid, informative outcome — not a reason to move goalposts.
- **Enrollment contamination:** if the early centroid is slightly mixed, enrollment leaks the absorber and extraction recovers him too. Mitigation: strict cos≥0.7 enrollment filter (tighter than the 0.5 used for profiling).
- **2-speaker vs 3-speaker:** late half is effectively 2-remote (UserA quiet per D8) → favorable for 2-mix-trained TSE. Early half has UserA active → do NOT measure PRIMARY on early half (would conflate 3-speaker failure with the real test).
- **Pitch-doubling / codec artifacts** in late half may produce voiced output that is neither UserB nor Speaker 2 → surfaces as cos<0.3 to both centroids; counted under the domain-mismatch outcome.

## Decision gate (post-spike)
- **PRIMARY passes** → `/opsx:propose` production TSE (Rust ONNX adapter behind a `SourceSeparationPort`, wired pre-embedding in the diarization pipeline). Full shark-tank workflow on that proposal.
- **DOMAIN-MISMATCH outcome** → separation-layer path closed. Write up; recommend per-participant capture as the only remaining fix (scope: platform SDK integration, months, platform-specific) or accept the limit for cde5c264.
- **TIEBREAKER (near-silence)** → H2 confirmed; cde5c264 was never recoverable; absorption narrative was wrong; update root-cause memory accordingly.

## Execution entries

### S1 — Stage 1 (embed + cluster + identify) — 2026-07-16
- **v1 FAILED:** chunked by Whisper transcript segments (avg ~21s, multi-speaker). Blended embeddings collapsed to 1 giant cluster (236/238). Wrong unit — production uses ~1.5s native windows.
- **v2 (fix):** 2s windows / 1.5s hop, early half only, skip RMS-silence → 934 speech windows embedded with production titanet (sherpa-onnx Python, 192-dim, identical to STEP 7).
- **3 balanced clusters (377 / 422 / 135).** F0 medians: **cl1=235 Hz** (UserB — matches D9's ~241), cl2=138 Hz, cl3=118 Hz. Two low-register clusters (likely UserA vs Speaker-2; not resolved — irrelevant to PRIMARY).
- Enrollment: UserB clean (cos≥0.7) = 206 windows → 28s ref WAV. Absorber (cl3) = 78 windows → 28s ref WAV.
- AHC is average-linkage, maxclust=3 (max_speakers=3, matches cde5c264 production).

### S2 — Stage 2 (VoiceFilter extraction + measurement) — 2026-07-16
20 late-half windows × 12s (1800s..4973s), VoiceFilter (egorsmkv, VoiceSplit-MSE-GE2E checkpoint), enrollment = clean early ref WAV. Measurement via **independent** titanet (not VoiceFilter's d-vector — avoids circularity).

| Metric | Value | Criterion | Verdict |
|---|---|---|---|
| PRIMARY: frac extracted(Cyn) cos≥0.5 to UserB | **65%** (median cos **0.689**) | ≥50% | **PASS** |
| Baseline: mixed-no-TSE cos≥0.5 to UserB | 50% (median 0.497) | — | context |
| Domain-mismatch: extracted(Cyn) cos to ABSORBER | median **0.036** | low to both = mismatch | **MISMATCH DISPROVEN** |
| Cross-check: extracted(Abs) cos to absorber / UserB | 0.505 / **0.068** | high own / low cross | **discriminating** |
| SECONDARY: UserB late duration (sample) | 156s of 240s sampled | ≥400s (extrapolates ~2060s) | passes on intent |
| H1/H2 | coherent UserB speech, not silence | — | **H1 favored** |

**Key positive:** VoiceFilter (synthetic-trained) transferred cleanly to real Zoom-mixed audio — the **enrollment cue overcomes the domain gap** where blind SepFormer failed (D7). This contradicts the pessimistic prior and the REAL-TSE baseline-ranking concern. Extraction is speaker-discriminative (0.689 to target vs 0.036 to other), not artifacts.

**Key caveat:** the mixed-no-TSE baseline is already 50% / median 0.497 — Python does **not** reproduce the production contamination (consistent with D10). TSE's marginal lift over baseline is therefore modest HERE (+15pp, +0.19 median cos). The spike proves TSE is viable and discriminating, but does not directly prove it fixes the Rust-pipeline absorption (that failure isn't reproduced in this Python measurement).

**Speed:** 14.3s per 12s window (Griffin-Lim) on CPU — ~1.2× realtime, offline-only. Full late-half UserB+absorber extraction ≈ ~2h CPU. Time-domain TSE (SpEx, no Griffin-Lim) or GPU would be needed for faster production use.

## Verdict & decision
- **TSE is a VALIDATED fix path.** Enrollment-conditioned extraction recovers a clean UserB signal from real mixed audio, disproving domain mismatch. This clears the bar to propose production integration.
- **BUT the spike re-surfaces a cheaper unresolved question:** Rust production contaminates UserB's late embeddings (STEP 7: 96.6% at cos<0.3) while Python on the **same model + same audio** does not (baseline 50% at cos≥0.5). Same titanet, same recording — different result. If the production absorption is a Rust-pipeline bug (chunking, stream source, sherpa-onnx Rust-vs-Python preprocessing), fixing it could be far cheaper than adding a TSE front-end. **This discrepancy should be investigated BEFORE committing to TSE integration.**
- **Recommendation:** before `/opsx:propose` for TSE, run a focused diagnostic comparing Rust native-window embeddings vs Python embeddings on identical chunks of cde5c264. If they diverge, the Rust bug is the real root cause (and TSE becomes unnecessary or a fallback). If they match, then the production audio stream itself differs from the saved recording and TSE pre-embedding is the fix.
- Diagnostic WAVs for user ground-truth listen: `mixed_2940.wav` / `extracted_cyn_2940.wav` (~49:00), `mixed_4020.wav` / `extracted_cyn_4020.wav` (~67:00) in scratchpad.

## S3 — Rust-vs-Python discrepancy RESOLVED: the "embedding failure" was a centroid artifact (2026-07-16)

The S2 recommendation was to investigate why Rust STEP 7 (96.6% of UserB's late speech cos<0.3) doesn't reproduce in Python (50% cos≥0.5). **Resolved: it was never an embedding failure. The diagnostic misidentified UserB's cluster.**

**Probe — 4 granularities, same titanet model, same saved recording, Python sherpa-onnx 1.13.3:**

| granularity | n_late | %cos<0.3 (dur-wt) | %cos≥0.5 | med cos | max cos |
|---|---|---|---|---|---|
| native (Rust cache windows) | 804 | **56.9%** | **41.5%** | 0.135 | 0.889 |
| fixed 2s/1.5s-hop | 1994 | 55.7% | 39.9% | 0.149 | 0.873 |
| fixed 12s | 264 | 51.9% | 41.7% | 0.219 | 0.888 |
| **prod (build_chunks replication)** | 430 | **53.8%** | **43.1%** | 0.182 | 0.903 |

Reference: Rust STEP 7 (native) = 96.6% cos<0.3, ~1.5% cos≥0.5. **Same windows (from Rust's cache file), same model — Python shows 41.5% UserB signal late, not 1.5%.**

**Centroid confounder ruled out:** a granularity-matched early centroid (cos 0.984 to spike_cen) gives identical late results as spike_cen. sherpa-onnx versions match (Python 1.13.3 / Rust 1.13). The gap is not the centroid *quality within Python* and not a version skew.

**Root cause — Rust diagnostic's STEP 1 misidentified UserB's cluster.** STEP 1 builds `cynthia_centroid` from `old_chunks` (prod build_chunks) clustered at 0.40, picking the **early-dominant** cluster (≥5 early chunks, fewest late). Reconstructing that exactly in Python (`rust_centroid_test.py`): the early-dominant cluster is cluster 1 (141 early / 7 late chunks, 985s early), and its centroid has **cos 0.179 to the validated spike_cen** — nearly orthogonal, a blended mush (cos 0.20 to the absorber too). That is NOT UserB; it's a UserA/blend cluster. Measuring late native windows against this mush reproduces STEP 7 exactly: **96.5% cos<0.3, 1.5% cos≥0.5** (Python embeddings). So STEP 7's "embedding failure" was measuring the wrong cluster's centroid.

**Decisive check — batch AHC does NOT absorb UserB when she is identified by centroid quality** (`native_ahc_check.py`, UserB = cluster with highest cos to spike_cen):

| granularity | thr | UserB cluster cos2cyn | early | late | late/early |
|---|---|---|---|---|---|
| native | 0.40 | **0.978** | 484s | 1063s | 2.20 |
| native | 0.50 | 0.982 | 449s | 1026s | 2.29 |
| prod | 0.40 | 0.935 | 557s | 1268s | 2.28 |

UserB's late speech is ~2× her early speech in every case. She does not vanish. The absorber is a separate cluster (cos 0.96 to absorber centroid). The "native windows still absorb" disproof (STEP 6/7) is invalid — it measured against the misidentified centroid.

**Consequences:**
- The "embedding-level failure / information-theoretic limit / cde5c264 unfixable" conclusion is **DISPROVEN**. The audio was always recoverable; the embeddings were clean.
- SIX methods (STEP 7/8, EEND, SepFormer, F0 ×2) were "disproven" against the contaminated centroid. EEND/SepFormer may still have genuine domain mismatch, but they were pursuing a non-problem.
- **TSE is NOT needed.** The embeddings already separate the speakers.
- **Production DB has NULL speaker labels** for cde5c264 (`speaker_source` all NULL) — diarization was never persisted. So no absorption claim comes from production output; every claim traces to the diagnostic tests.

**OPEN — does the FULL production pipeline (AHC + temporal smoothing + coalesce + merge_short) still absorb UserB in the live UI?** Batch AHC alone does not. If the user's "UserB vanishes after min 30" observation holds on *current* production, the mechanism is in the smoothing/coalesce/merge passes (not yet tested with correct speaker ID), or the observation was from a pre-flicker-fix pipeline version. Re-running `stage_trace_diagnostic` with UserB identified by spike_cen (not the early-dominant heuristic) would settle this. Scratchpad: `rustparity_native.py`, `centroid_analysis.py`, `rust_centroid_test.py`, `native_ahc_check.py`, `parity_emb.npz`.

## S4 — Full production pipeline replication: NO ABSORPTION at any stage (2026-07-16)

Ported all four `process()` stages to Python on cached prod embeddings (`full_pipeline.py`), identifying UserB by spike_cen at each stage. UserB = cluster with centroid cos 0.924-0.927 to spike_cen throughout:

| stage | UserB early | UserB late | late/early | cos2cyn |
|---|---|---|---|---|
| A. cluster_by_centroids (AHC) | 557s | 1268s | 2.28 | 0.927 |
| B. + smooth_to_fixed_point | 525s | 1351s | 2.57 | 0.924 |
| C. + coalesce → segments | 545s | 1489s | 2.73 | 0.924 |
| D. + merge_short (FINAL) | 545s | 1492s | 2.74 | 0.924 |

UserB's late duration **grows** through the pipeline (1268 → 1492s). She is never absorbed. The absorber (cluster 2, cos 0.897 to absorber centroid) stays a distinct cluster at every stage. `merge_short_speakers`'s floor is 98s (2% of ~4900s); UserB's 2037s total is far above it, so she is never merged away.

The speaker that *does* vanish is **cluster 1: 1049s early → 42s late (ratio 0.04), cos 0.21 to spike_cen** — the early-dominant cluster the diagnostic's heuristic mislabeled "UserB." It is NOT UserB; it is the local-mic/UserA speaker (quiet late, consistent with D8). The user's "UserB vanishes after min 30" observation most likely tracked this wrong cluster, or dates from a pre-temporal-smoothing pipeline version.

**Verdict: the absorption problem does not exist in the current production pipeline.** The entire absorption narrative — embedding failure, information-theoretic limit, six disproven methods, TSE spike — was built on a contaminated centroid produced by the diagnostic's own early-dominant speaker-identification heuristic. The embeddings, clustering, smoothing, coalescing, and merge passes all work correctly; UserB is attributed ~1492s of late speech.

**Caveat:** this is a faithful Python port of the Rust pipeline (AHC validated to reproduce `cluster_by_centroids` exactly; smoothing/coalesce/merge translated from `sherpa_adapter.rs` source), not the Rust binary itself. The authoritative confirmation is to run the real `process()` and inspect final labels — but since the DB has NULL persisted labels for this meeting, that requires a new `#[ignore]` diagnostic that exports final segments + centroids, or re-running `stage_trace_diagnostic` re-identified against spike_cen. Recommend that confirmation before formally closing the thread, but the direction is unambiguous across all four stages.

## S5 — Rust-binary confirmation: NO ABSORPTION (authoritative, 2026-07-16)

Added `#[ignore]` test `test_cde5c264_export_final_pipeline` that runs the **real** `adapter.process()` + `enforce_max_speakers_cap` on cde5c264's saved audio and exports the final 89 segments + 3 centroids to `%TEMP%/meetily_final_labels_cde5c264.txt`. Analyzed in Python against spike_cen (`analyze_rust_export.py`):

| speaker | early | late | late/early | cos2cyn | cos2abs |
|---|---|---|---|---|---|
| **0 (UserB)** | 545s | **1521s** | 2.79 | **0.923** | 0.067 |
| 1 (vanishing) | 1062s | 42s | 0.04 | 0.215 | 0.202 |
| 2 (absorber) | 178s | 1545s | 8.67 | 0.094 | 0.899 |

**Authoritative verdict: UserB is NOT absorbed.** 1521s of late speech attributed to her (speaker 0, centroid cos 0.923 to the validated spike_cen). Matches the Python replication (545/1492) to within segment-coalescing noise — the early duration is identical to the second (545s). The speaker that vanishes (1062s early → 42s late, cos 0.215 to UserB) is the local-mic/UserA cluster, not UserB; this is the cluster the diagnostic's early-dominant heuristic mislabeled "UserB," generating the entire absorption narrative.

**THREAD CLOSED.** The diarization absorption problem does not exist in the current production pipeline. Downstream consequences:
- `diarization-segmentation-windows` and `diarization-f0-correction` changes rest on disproven premises — repurpose or abandon.
- `test_cde5c264_native_pipeline_diagnostic`'s STEP 1 early-dominant heuristic is the bug that generated the false absorption signal; it should be fixed (identify speakers by centroid quality, not early-dominance) or the test removed to prevent re-confusing future work.
- TSE / EEND / SepFormer integration is unnecessary.
- Scratchpad spike artifacts can be deleted; F0 code stays in the repo behind `F0_CORRECTION_ENABLED=false` per prior instruction.
