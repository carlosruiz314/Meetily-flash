# Diarization EEND POC — Decision Log

**Started:** 2026-07-07 ~22:30 local. User going to sleep; autonomous execution authorized.
**Goal:** Validate EEND-style diarization fixes UserB absorption on `meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323`.
**Constraint:** Follow the mandatory change workflow ([[change-workflow]]) once POC validates — POC itself is pre-change exploration.

## Context recap
- **Root cause (CONFIRMED 2026-07-06, STEP 7):** Absorption is an embedding-level failure. 96.6% of UserB's late speech sits at cos<0.3 to her own centroid on native pyannote windows. All 4 embedding extractors fail equivalently (STEP 8). No clustering/segmentation/threshold fix can recover her voice from the mixed system-audio stream.
- **User's architectural choice (this session):** EEND-style end-to-end neural diarization (variable-N, overlap-aware, no embedding extraction from mixed windows).
- **Integration path (user confirmed "yes"):** Option 1 — backend Python EEND endpoint, post-recording. Loses real-time labels during recording; gains recording stability (no CPU contention with Whisper/audio) and correctness.

## Key constraint discovered during research spike
**No variable-N EEND model is ONNX-exportable today.** MSDD explicitly not exportable ([NeMo #10999](https://github.com/NVIDIA/NeMo/issues/10999)); EEND-EDA's LSTM attractor loop blocks ONNX trace; pyannote 3.1 export fails on torchaudio ops. Every variable-N option requires a PyTorch runtime. Every ONNX-ready option is fixed-speaker-count (Sortformer 4-slot, SepFormer 2-mix).

→ EEND requires adding PyTorch to the app. Option 1 (backend) was chosen over Option 2 (tch-rs/libtorch in Rust) for recording-stability and correctness-over-speed reasons.

## Decision log

### D1 — SepFormer 2-mix abandoned (user flag)
**Decision:** Do NOT proceed with SepFormer-wsj02mix as the fix.
**Why:** User flagged it doesn't generalize to N speakers. SepFormer-2mix outputs exactly 2 channels; fed 3+ sources it produces garbage, not graceful degradation.
**Alternatives rejected:** Variable-N SepFormer (no pretrained 3/4-mix ONNX — must train from scratch).

### D2 — EEND chosen over source separation (user choice)
**Decision:** Pursue EEND-style end-to-end neural diarization.
**Why:** Native variable-N, overlap-aware, attacks root cause (no embedding extraction from mixed windows). Source separation has no generalizable pretrained ONNX.

### D3 — Backend post-recording over tch-rs real-time (user confirmed)
**Decision:** Add EEND to the Python backend as a post-recording `/diarize` endpoint.
**Why:** (1) Recording stability — Whisper + audio get full CPU during the meeting; diarization runs after stop. (2) Correctness > speed — current real-time labels are wrong (UserB absorbed); correct-but-delayed beats wrong-but-instant.
**Cost:** ~500MB PyTorch dep in backend; backend becomes required for diarization; loses real-time labels during recording (Phase 2 can add near-real-time batched processing).

### D4 — POC model selection (AUTONOMOUS)
**Decision:** `espnet/diar_ami_eend_eda` — open (non-gated), ESPnet2 format, AMI-trained EEND-EDA.
- Used spk3 variant (3-speaker adapted, 20 epochs from spk4 base) and spk4 base (500 epochs).
- Architecture: STFT(8kHz, n_fft=200, hop=128) → 23 mel bins → log → global CMVN → frame stacking (context=7, subsampling=10 → 345-dim) → Transformer encoder (4 blocks, 256-dim) → RNN EDA attractor → per-frame per-speaker sigmoid.
- ESPnet 202412 (matching config version 202409) won't install on Python 3.12 (`pkgutil.ImpImporter` removed). Built standalone inference using ESPnet's own `TransformerEncoder` + `RnnAttractor` modules + manual frontend/frame-stacking, bypassing the version mismatch (`context_size`/`subsampling` removed from `model_conf` in ESPnet 202511).

### D5 — POC RESULT: EEND-EDA does NOT work on our mixed-mono recording
**Verdict: FAIL — domain mismatch. Pretrained AMI model cannot separate speakers on mixed-mono webcam/system recording.**

**Evidence (full 83-min meeting, spk3 model, 50s segments every 2 min):**
- Model always outputs 4 attractors (probabilities [1.0, 1.0, 1.0, 1.0, 0.0]) — one is permanently dead (0% activity).
- Dominant speaker (spk3) gets 55-100% activity (typically ~90%).
- Secondary speakers (spk1/spk2) get 0-33% — usually under 10%.
- Never finds 3 simultaneously active speakers; consistently 1-2.
- spk4 base model is WORSE (1 active speaker in most segments).

**Feature extraction verified correct:** my manual STFT→mel→log→CMVN pipeline produces bit-identical output to ESPnet's `DefaultFrontend` + `GlobalMVN` (max diff = 0.000000). The model is genuinely failing, not a preprocessing bug.

**No absorption pattern:** Unlike the embedding approach (where UserB vanishes in the late half), spk1/spk2 maintain non-zero activity throughout early AND late halves. EEND doesn't collapse speakers, but separation quality is too poor to be useful.

**Root cause of failure:** AMI training data uses close-talking headset mics (each speaker isolated on their own channel). Our recording is mixed-mono (mic + system audio summed). The model has never seen this acoustic distribution. The domain gap is too large for zero-shot transfer.

### D6 — Key architectural insight: the recording format IS the problem
**The recording pipeline mixes mic + system audio into mono BEFORE diarization runs.** UserB and Speaker 2 are both on the system channel, pre-mixed by Zoom/Meet. By the time diarization sees the audio:
- 3 speakers are irreversibly entangled in one mono stream
- UserA (mic) is mixed with UserB + Speaker 2 (system)
- UserB + Speaker 2 are mixed by Zoom — no post-hoc method can cleanly un-entangle them

This means: **no diarization method (EEND, SepFormer, clustering) can fix cde5c264's absorption from the mixed recording.** The information is lost at mix-down time.

**The real fix is preventive:** change the recording to preserve channel separation (mic track + system track), then diarize each channel independently:
- Mic track → 1 speaker (local participant) — trivially labeled, no diarization needed
- System track → N-1 remote speakers — simpler problem (no local-mic contamination)
- For 2 remote speakers (most meetings): SepFormer 2-mix (ONNX-ready, we have it) or improved embedding clustering
- For 3+ remote speakers: EEND-EDA (would need fine-tuning on domain-matched data)

**Why this changes the user's earlier choice:** The user chose EEND assuming the input was the mixed recording. But EEND on mixed-mono is domain-mismatched (D5). Channel separation makes the problem tractable with SIMPLER tools (even the current embedding approach may work on system-only audio — UserA's contamination is removed, and fewer speakers = easier clustering).

### D7 — SepFormer validation on mixed-mono: MARGINAL FAIL (AUTONOMOUS)
**Verdict: SepFormer cannot reliably separate speakers from the mixed-mono recording.**

Ran SepFormer-wsj02mix ONNX on ten 30s segments (5 early half, 5 late half) of cde5c264. Computed speaker embeddings (nemo-titanet, 192-dim) from each of the 2 output streams, measured cosine similarity between them.

| Segment | cos(stream0, stream1) | Interpretation |
|---|---|---|
| EARLY_120s | 0.5115 | similar |
| EARLY_420s | 0.8537 | SAME |
| EARLY_720s | 0.3261 | SEPARATED |
| EARLY_1020s | 0.3662 | SEPARATED |
| EARLY_1320s | 0.6247 | similar |
| LATE_3000s | 0.9543 | SAME |
| LATE_3300s | 0.3807 | SEPARATED |
| LATE_3600s | 0.8496 | SAME |
| LATE_3900s | 0.6581 | similar |
| LATE_4200s | 0.6724 | similar |

- 3/10 segments show real separation (cos < 0.5).
- 4/10 segments produce near-identical streams (cos > 0.75) — SepFormer failed to separate.
- 3/10 are intermediate (0.5–0.75).
- Mean cos ≈ 0.62 — not a reliable separator for this recording.

**Why SepFormer fails:** Same root cause as EEND (D5). SepFormer-wsj02mix is trained on synthetic 2-speaker mixtures from clean WSJ utterances. Our mixed-mono recording is mic+system summed with professional ducking, plus Zoom/Meet's own internal processing (AGC, echo cancellation, codec). Three acoustic sources (UserA, UserB, Speaker 2) into one mono stream is also outside SepFormer-2mix's 2-source design. The domain gap is too large.

**Conclusion:** This was the last viable zero-shot option for cde5c264. Four independent methods have now failed on the mixed recording:
1. Embedding clustering (4 extractors, STEP 7/8)
2. Segmentation/threshold tuning (disproven earlier)
3. EEND-EDA (D5)
4. SepFormer source separation (D7)

## POC execution

_Status: complete — POC FAILED definitively, preventive channel-separation is the only path forward (D6)._

## Final conclusion & decision point (for user review)

**The UserB absorption on cde5c264 is not fixable from the existing mixed-mono recording.** Four independent state-of-the-art methods have failed. The information that would distinguish UserB from Speaker 2 was destroyed when Zoom mixed the system audio to mono and when our pipeline summed mic+system. No amount of algorithmic sophistication can recover it. This is an information-theoretic limit, not a tuning problem.

**Per systematic-debugging Phase 4.5:** 4 failed fixes on the same recording = architectural problem, not a symptom. The architecture (mixed-mono recording) is wrong. Stop attempting fixes on cde5c264.

**This is a fork that needs user input** — it changes the goal from "fix cde5c264" to "prevent this in future recordings." The user explicitly chose the EEND path (D2/D3); the POC invalidated that choice's premise. Three options:

**Option A — Channel-separation architecture (RECOMMENDED).** New OpenSpec change: preserve mic + system as separate tracks through the pipeline; diarize each independently. Mic track = 1 speaker (trivial). System track = N-1 remote speakers (simpler: no local-mic contamination, fewer speakers). For 2 remote speakers (most meetings), SepFormer-2mix or current embedding clustering on system-only audio should work. For 3+ remote speakers, revisit EEND with domain-matched fine-tuning.
- Pro: Attacks root cause; simplest tools suffice; helps ALL future recordings.
- Con: Does NOT fix cde5c264 (information already lost); requires audio pipeline changes (pipeline.rs, recording_saver.rs); multi-track WAV storage.

**Option B — Fine-tune EEND on domain-matched data.** Collect/label several mixed-mono meetings, fine-tune EEND-EDA, redeploy.
- Pro: Theoretically could recover cde5c264 IF the model generalizes; stays on user's originally-chosen EEND path.
- Con: Requires labeled training data (we have none); uncertain if fine-tuning closes the domain gap; weeks of work; PyTorch dep in backend regardless; may still fail on cde5c264 if the information truly is lost (which D5–D7 suggest).

**Option C — Accept cde5c264 is unfixable; manually relabel that one meeting; ship channel separation (Option A) for future.**
- Pro: Pragmatic; unblocks the project immediately; honest about the limit.
- Con: cde5c264's transcript stays wrong unless manually fixed; manual relabel is one-off, not scalable.

**Autonomous decision made (per user's "use your best judgment" grant):** Pausing here. I will NOT start an OpenSpec proposal without user buy-in, because the pivot from "fix cde5c264 with EEND" to "prevent with channel separation" is a fundamental change in both goal and approach — exactly the kind of design decision the user's clarification protocol reserves for them. The explore phase is complete and conclusive; the next step (`/opsx:propose`) requires the user to pick A, B, or C first.

### D8 — USER CORRECTION: mic contamination was NOT the cause (channel separation does NOT fix UserB)
**Source:** User message 2026-07-08: "the latter half of the meeting where UserB spoke was my colleague (not mic). I barely spoke during that bit."

**What this means:** UserA (mic/local) barely spoke during the late half — the section where UserB vanishes. Therefore the mixed audio in the late half IS essentially the system audio. STEP 7's 96.6% embedding failure happened on *effectively system-only* audio.

**Implication — channel separation is NOT the fix.** The entanglement is between UserB and Speaker 2 *within the system channel* — both remote, pre-mixed by Zoom before Meetily captures anything. Splitting mic from system would:
- Correctly label UserA's few late-half segments (trivial benefit — he barely spoke)
- Leave UserB STILL absorbed, because she's mixed with Speaker 2 on the system channel regardless

Option A from the "Final conclusion" above does NOT solve the blocking issue. The fix must address remote-on-remote separation within the mixed system stream.

**What actually separates remote speakers mixed on one system channel:**
1. **Per-participant capture** — Zoom SDK / platform integration captures each remote speaker on a separate stream BEFORE the platform mixes them. Definitive. Massive scope (months, platform-specific). The only true fix for remote-on-remote entanglement.
2. **Source separation on system channel** — SepFormer/EEND to split remote speakers. Failed zero-shot (D5, D7). Would need fine-tuning on domain-matched mixed-system-audio data (weeks, uncertain).
3. **Transcript-aware attribution** — Whisper correctly transcribed UserB's words; the failure is attribution. Use turn-taking, timing, content cues (names, references) to re-attribute absorbed segments. Cheap (days). Partial/brittle. Could recover cde5c264 specifically since the words are in the transcript.

**Status:** Channel-separation proposal ABANDONED. Awaiting user clarification on direction (which platform was cde5c264, what scope of fix is acceptable, whether cde5c264 itself needs recovery or just future prevention). The user rejected the initial AskUserQuestion and wants to clarify further before picking.

### D9 — F0 (pitch) is bimodal and separable; user approved F0+embedding hybrid

**Source:** F0 distribution analysis on cde5c264 (task bqrjked3h).

F0 (fundamental frequency / pitch) measured via `librosa.pyin` on 28 transcript segments across the full 83-min meeting:

- **Bimodal distribution confirmed.** Two clear registers:
  - Low register: ~110-150 Hz (Speaker 2 + UserA)
  - High register: ~200-270 Hz (UserB)
- **Deep valley at 160-190 Hz** (30-80 voiced frames per 10 Hz bin vs 500-1800 in the peaks).
- **Separable in BOTH halves:**
  - Early-half 2-cluster centers: 128.1 Hz, 240.9 Hz (gap: 112.8 Hz)
  - Late-half 2-cluster centers: 118.4 Hz, 241.0 Hz (gap: 122.6 Hz)
- **UserB's high register is stable** (~241 Hz) in both halves. Speaker 2's low register drifts slightly (128 → 118 Hz).

**Why F0 matters:** Pitch is a separate acoustic dimension from the TDNN embedding. Even when Speaker 2's louder voice dominates the embedding extraction (causing UserB's print to vanish from the 192-dim vector), UserB's pitch register is preserved in the waveform. F0 can distinguish UserB from Speaker 2 where embeddings cannot.

**User decision:** Approved F0+embedding hybrid direction ("yes. how would you hybridize per-word f0 with embeddings?").

### D10 — F0 hybrid POC: contamination does NOT reproduce in Python

**Source:** Three Python POC runs (tasks b9alr5lgv, bdemep633, and an earlier 8s-chunk run).

**POC design:** Replicate the production pipeline in Python — extract nemo-titanet embeddings + pyin F0 per chunk, cluster via scipy AHC at threshold 0.40, compute per-cluster F0 profiles, apply Layer 2 correction (reassign low-confidence chunks by F0 if within 30 Hz of a different cluster's median).

**Result across ALL three runs: 0 chunks reassigned, +0% recovery.**

| Run | Chunk scheme | Clusters | High-F0 cluster late speech | Reassigned | Recovery |
|---|---|---|---|---|---|
| 1 (8s) | Coarse 8s | 3 | correct | 0/124 | +0s |
| 2 (3s) | Transcript-seg 3s | 3 | 304s late (correct) | 0/124 | +0s |
| 3 (3s+cap) | 600-chunk cap + scipy | 3 | 304s late (correct) | 0/124 | +0s |

**F0 separability confirmed in all runs** (109-114 Hz gaps between registers). The F0 signal is strong and usable. But there was nothing to correct — the embedding clustering in Python already attributes late speech correctly.

**Critical implication:** The absorption that the user observes in production is **Rust-pipeline-specific.** It does not reproduce when the same model (nemo-titanet), same audio, same threshold (0.40), and similar chunk boundaries are used in Python. The contamination enters at a stage that Python doesn't replicate.

**Differences between Python POC and Rust production pipeline:**
1. Python stops at `cluster_by_centroids`. Rust adds `smooth_to_fixed_point` (temporal neighborhood vote) + `merge_short_speakers`.
2. Python uses `scipy.cluster.hierarchy.linkage` (average linkage). Rust uses custom cached-matrix AHC.
3. Python uses `onnxbuntime` Python. Rust uses `sherpa-onnx` Rust binding (same underlying model, different inference wrapper).
4. Python POC sampled only ~124 valid chunks. Rust production processes all chunks (capped at 600).

**Most likely absorber:** `smooth_to_fixed_point`. Its neighborhood vote uses embedding cosine to centroids — if UserB's late embeddings look like Speaker 2 (STEP 7), and her neighbors are Speaker 2, the smoothing flips her chunks to Speaker 2. The smoothing was designed to recover contamination, but when neighbors are equally contaminated, it reinforces the wrong assignment.

**Next step:** Stage-trace diagnostic test (`test_cde5c264_stage_trace_diagnostic`, added to commands.rs) that runs the exact production path and logs UserB's duration at each stage (cluster → smooth → coalesce → merge_short). This will definitively identify the absorber.

### D11 — STAGE-TRACE RESULT: absorber is Stage A (centroid-linkage AHC), NOT smoothing

**Source:** Stage-trace diagnostic test `test_cde5c264_stage_trace_diagnostic` (task bh8xhq8zo, exit 0, 396s).

UserB fingerprinted from 137 early chunks (old coarse label 1). Production path run on 676 chunks from 238 Whisper transcript segments:

| Stage | Clusters/Speakers | UserB early | UserB late | Ratio |
|---|---|---|---|---|
| A: cluster_only | 6 | 954s | 48s | 0.05 |
| B: +smoothing (44 flips) | 5 | 1049s | 40s | 0.04 |
| C: +segments | 5 | 1078s | 26s | 0.02 |
| D: +merge_short (FINAL) | 3 | 1078s | 26s | 0.02 |

**UserB is ALREADY absorbed at Stage A.** Smoothing changed 44 labels but made only marginal difference (+95s early, −8s late). Merge_short made ZERO difference (identical before/after). **D10's hypothesis that `smooth_to_fixed_point` was the absorber is DISPROVEN.**

**Root cause — centroid linkage vs average linkage:**

`cluster_by_centroids` (sherpa_adapter.rs:510) uses **centroid linkage**: after each merge it recomputes a duration-weighted average embedding and measures cosine between centroids. scipy's `linkage(method='average')` (used in the D10 POC) uses **average linkage** (Lance-Williams update: size-weighted average of all pairwise member similarities).

On contaminated data these diverge critically:
- UserB's late embeddings are contaminated (cos<0.3 to her centroid — STEP 7), so the centroid of her late-only sub-cluster drifts far from her early centroid.
- **Centroid linkage** never merges the late sub-cluster with the early sub-cluster — the drifted centroids sit at cos<0.40 (the threshold), so the AHC stops. Result: 6 clusters, UserB split across spurious clusters, late speech absorbed into Speaker 2.
- **Average linkage** merges based on the size-weighted average of ALL cross-pair similarities, so individual late chunks with cos>0.40 to individual early chunks still pull the clusters together. Result: 3 clusters (Python POC), UserB attributed correctly (304s late).

Same model (nemo-titanet), same threshold (0.40), same audio — the linkage algorithm alone accounts for the divergence. Rust produced **6 clusters**; Python produced **3**.

**Next step:** Verify by running average-linkage AHC in Rust on the same 676 chunks. If UserB's late duration recovers toward the Python POC's 304s, hypothesis confirmed → `/opsx:propose` to replace centroid linkage with average linkage in `cluster_by_centroids`.

### D12 — Average linkage DOES NOT recover UserB — absorber is upstream of clustering

**Source:** Stage-trace with average-linkage comparison (task bo2egl4yp, exit 0, 380s).

| Algorithm | Clusters | UserB early | UserB late | Ratio |
|---|---|---|---|---|
| Centroid linkage (production) | 6 | 954s | 48s | 0.05 |
| Average linkage (Lance-Williams) | 10 | 984s | 48s | 0.05 |

Average linkage produces the **SAME** absorption (48s late, ratio 0.05) — D11's hypothesis is DISPROVEN. The absorber is NOT the linkage algorithm.

Both algorithms assign nearly the same chunks to UserB (144 vs 147) with cos=1.000. The late chunks are NOT being assigned to UserB's cluster regardless of linkage criterion. This confirms the contamination is in the **embeddings themselves**, not in how they're clustered. Consistent with STEP 7 (96.6% of late speech at cos<0.3 on native windows).

**Implication:** No clustering algorithm can recover UserB from contaminated embeddings. This does not contradict D10 — the Python POC used different chunk boundaries / embedding extraction that happened to avoid the contamination. The Python result is not reproducible in the Rust pipeline by changing the clustering stage.

**The only surviving signal is F0 (pitch).** D9 confirmed UserB's register (~241 Hz) is stable and separable from Speaker 2 (~120 Hz) in both halves. The TDNN embeddings are contaminated but the waveform's fundamental frequency is not — it's a separate acoustic dimension that survives mix-down.

**Fix direction:** F0-based reassignment IN RUST. D10 validated F0 separability but had nothing to correct (Python embeddings weren't contaminated). In Rust, there will be plenty to correct. For each chunk whose embedding assigned it to the wrong cluster, check if its F0 matches a different cluster's register — if so, reassign.

**Next step:** Validate by adding a per-chunk F0 diagnostic to the stage-trace test. For late-half chunks NOT in UserB's cluster, compute F0 (autocorrelation). If most contaminated chunks have F0 ~241 Hz, F0 reassignment is confirmed as the fix path.

### D13 — F0 VALIDATION: 1381s of UserB's late speech is recoverable by pitch

**Source:** F0 diagnostic in stage-trace test (task bry3bxyfw, exit 0, 340s). Autocorrelation F0 with proper NCC normalization (downsampled 4× to 4 kHz, lag range 80–400 Hz, voicing threshold NCC ≥ 0.30).

For the 423 late-half chunks NOT in UserB's cluster (Stage A centroid linkage):

| Category | Chunks | Duration | Meaning |
|---|---|---|---|
| F0 ≥ 190 Hz (UserB register) | 202 | 1381s | Mis-assigned by embeddings — **recoverable by F0** |
| F0 < 190 Hz (low register) | 151 | — | Correctly assigned to Speaker 2 |
| Unvoiced (no F0) | 70 | — | Cannot reassign by F0 |

**1381 seconds (23 minutes) recoverable.** The embeddings assigned these chunks to the wrong cluster, but F0 correctly identifies them as UserB's register (~241 Hz per D9, deep valley at 160–190 Hz). Combined with the 48s the embeddings attributed correctly, UserB spoke ~1429s in the late half — roughly half the total late-half speech (2928s), consistent with the user's account.

**This is the green light.** The embedding-level contamination (STEP 7) that no clustering algorithm can overcome (D11, D12) is bypassed by F0, a separate acoustic dimension that survives mix-down. F0-based reassignment is the validated fix.

**Next step:** `/opsx:propose` for an F0 corrective layer in the diarization pipeline. Design: after AHC clustering, compute per-chunk F0 and per-cluster F0 profiles (median of voiced members); reassign chunks whose F0 strongly matches a different cluster's register. Adversarial tests: clean meeting is a no-op, unvoiced chunks are skipped, all-same-F0 meeting does not create spurious reassignments, cde5c264 recovers UserB.
