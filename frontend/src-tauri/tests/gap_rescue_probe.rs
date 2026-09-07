//! Gap-rescue de-risk probe (change `gap-speech-voice-attribution`, tasks 1.1–1.6).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan --test gap_rescue_probe -- --ignored --nocapture
//! (runner: `.tools/run_rescue_probe.bat`)
//!
//! Answers, on real audio with ENGINE-REAL geometry:
//!   1.1  current turn set + pyannote votes for 12–17s / 32–38s + borrow distances at the target gap
//!   1.2  voiced-onset detection rehearsal inside the 14.78–16.12s gap (the mechanism depends on it)
//!   1.3  identity votes for the raw windows and the voiced sub-window (both transcript sources)
//!   1.4  whole-meeting candidate list under the FULL gate set, mapped to fixture entries
//!   1.5  controls: true-silence gaps under the same gates (false-rescue denominator)
//!   1.6  post-splice predicate rehearsal — replicate the piece list, splice AT THE ONSET,
//!        re-run resolve_turns, assert the S2b/S1/S3 predicates
//!
//! Everything here is measurement, not assertion: the printed GO/NO-GO record
//! goes into the change folder.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::{
    local_labels, FrameMassesOutput, PyannoteSegmentation, FRAME_SHIFT,
};
use app_lib::audio::speaker::run_assembly::{
    cluster_pieces, cosine, derive_pieces, drop_textless, embed_slice, margin_to_centroids,
    merge_to_cap, refine_loop, resolve_turns, speech_runs, PieceIn, AMBIGUITY_MARGIN,
    EMBED_FLOOR_SECS, MODE_FILTER_RADIUS_FRAMES, PIECE_CAP, PROMOTION_FLOOR_SECS, SPEECH_GATE,
    TEXT_SKEW_TOLERANCE_SECS,
};
use app_lib::audio::speaker::run_engine;
use serde::Deserialize;

const AUDIO: &str =
    "Music/meetily-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const MODELS_DIR: &str = ".meetily-models";
const SAMPLE_RATE: usize = 16_000;
const MEETING_CAP: usize = 3;
const MERGE_THRESHOLD: f32 = 0.65;
/// Borrow cap (commands.rs GAP_BORROW_MAX_MS).
const BORROW_CAP_MS: i64 = 3_000;
const RESCUE_MARGIN: f32 = AMBIGUITY_MARGIN;
const RAW_FLOOR_SECS: f64 = PROMOTION_FLOOR_SECS;
const GAP_END: f64 = 16.12;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

#[derive(Deserialize)]
struct Fixture {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    start_s: f64,
    end_s: f64,
    #[serde(default)]
    change_at_s: Option<f64>,
    #[serde(default)]
    tolerance_s: Option<f64>,
}

/// Point-to-turn-span distance in ms, containment-0 start-inclusive/end-exclusive.
fn dist_ms(p: i64, s: i64, e: i64) -> i64 {
    if p >= s && p < e {
        0
    } else if p < s {
        s - p
    } else {
        p - e
    }
}

/// Modeled borrow winner: nearest turn edge to `mid` (i64 ms), ties → earlier
/// turn, within the borrow cap. Returns (label, dist_ms).
fn borrow_winner(turns: &[(f64, f64, u32)], mid: i64) -> Option<(u32, i64)> {
    let mut best: Option<(u32, i64)> = None;
    for (s, e, l) in turns {
        let d = dist_ms(mid, (*s * 1000.0) as i64, (*e * 1000.0) as i64);
        if d > BORROW_CAP_MS {
            continue;
        }
        match best {
            // strict <: ties keep the earlier turn (list is time-ordered)
            Some((_, bd)) if d >= bd => {}
            _ => best = Some((*l, d)),
        }
    }
    best
}

/// Voiced-onset detection inside [a, b): 20ms frames, RMS dBFS; robust
/// baseline = median frame dB; onset = first frame sustaining baseline+12dB
/// for 3 consecutive frames (60ms). Returns (onset_s, baseline_db, peak_db).
fn detect_onset(samples: &[f32], a: f64, b: f64) -> (Option<f64>, f32, f32) {
    let hop = SAMPLE_RATE / 50; // 20ms
    let i0 = (a * SAMPLE_RATE as f64) as usize;
    let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
    let mut dbs: Vec<f32> = Vec::new();
    let mut j = i0;
    while j + hop <= i1 {
        let rms = (samples[j..j + hop].iter().map(|v| v * v).sum::<f32>() / hop as f32).sqrt();
        dbs.push(20.0 * rms.max(1e-10).log10());
        j += hop;
    }
    if dbs.is_empty() {
        return (None, -120.0, -120.0);
    }
    let mut sorted = dbs.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let baseline = sorted[sorted.len() / 2];
    let peak = dbs.iter().cloned().fold(f32::MIN, f32::max);
    let thr = baseline + 12.0;
    let mut run = 0usize;
    for (k, &d) in dbs.iter().enumerate() {
        if d >= thr {
            run += 1;
            if run >= 3 {
                let onset_frame = k + 1 - run;
                return (
                    Some(a + (onset_frame * hop) as f64 / SAMPLE_RATE as f64),
                    baseline,
                    peak,
                );
            }
        } else {
            run = 0;
        }
    }
    (None, baseline, peak)
}

#[tokio::test]
#[ignore = "live probe: MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan --test gap_rescue_probe -- --ignored --nocapture"]
async fn gap_rescue_signals() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();
    eprintln!("PROBE: decoded {:.1}s", decoded.duration_seconds);

    // Transcript spans from BOTH sources.
    let gate_json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(&audio_path)
                .parent()
                .unwrap()
                .join("transcripts.json"),
        )
        .expect("read transcripts.json"),
    )
    .expect("parse transcripts.json");
    let gate_spans: Vec<(f64, f64)> = gate_json
        .get("segments")
        .and_then(|v| v.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some((
                        r.get("audio_start_time")?.as_f64()?,
                        r.get("audio_end_time")?.as_f64()?,
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(format!("{}/{}", home(), DB_PATH)),
        )
        .await
        .expect("db connect");
    let db_rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time FROM transcripts \
         WHERE meeting_id = ? AND audio_start_time < audio_end_time ORDER BY audio_start_time",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("fetch db rows");
    let db_spans: Vec<(f64, f64)> = db_rows
        .iter()
        .map(|r| {
            let s: f64 = sqlx::Row::get(r, "audio_start_time");
            let e: f64 = sqlx::Row::get(r, "audio_end_time");
            (s, e)
        })
        .collect();
    let references = app_lib::database::repositories::speaker::SpeakerRepository::list_stamped_embeddings(&pool)
        .await
        .unwrap_or_default();
    drop(pool);
    eprintln!(
        "PROBE: spans gate={} db={} refs={}",
        gate_spans.len(),
        db_spans.len(),
        references.len()
    );

    // Models + cached masses + engine run (pre-rescue = current engine).
    let models_dir = format!("{}/{MODELS_DIR}", home());
    let pya = PyannoteSegmentation::new(&format!("{models_dir}/pyannote-segmentation.onnx"))
        .expect("pyannote model");
    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{models_dir}/{}",
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");
    let cache_path = std::path::Path::new(&audio_path)
        .parent()
        .unwrap()
        .join("gate_frame_masses.json");
    let prov = app_lib::audio::speaker::pyannote_segmentation::cache_provenance(
        std::path::Path::new(&format!("{models_dir}/pyannote-segmentation.onnx")),
    )
    .expect("provenance");
    let fm = match FrameMassesOutput::load(&cache_path, &prov) {
        Ok(fm) => fm,
        Err(_) => {
            let fm = pya.frame_masses(&samples).expect("frame masses");
            fm.save(&cache_path, &prov).expect("save cache");
            fm
        }
    };
    let out = run_engine::derive_turns_from_masses(
        &fm,
        &extractor,
        &samples,
        &gate_spans,
        MERGE_THRESHOLD,
        MEETING_CAP,
        &references,
    )
    .expect("engine run");
    let pre_turns: Vec<(f64, f64, u32)> = out
        .turns
        .iter()
        .map(|t| (t.start_seconds, t.end_seconds, t.speaker_id))
        .collect();
    eprintln!(
        "PROBE: engine pre-rescue: {} turns, {} clusters, {} refs",
        pre_turns.len(),
        out.centroids.len(),
        references.len()
    );
    let mut cids: Vec<u32> = out.centroids.keys().cloned().collect();
    cids.sort_unstable();
    for w in cids.windows(2) {
        if let (Some(a), Some(b)) = (out.centroids.get(&w[0]), out.centroids.get(&w[1])) {
            eprintln!("PROBE: inter-centroid cosine sp{}-sp{} = {:.3}", w[0], w[1], cosine(a, b));
        }
    }

    // ---- 1.1 turn dump + pyannote votes + borrow distances ----
    eprintln!("\n=== 1.1 turns 12-17s and 32-38s ===");
    for (s, e, l) in &pre_turns {
        if (12.0..=17.5).contains(s) || (31.5..=38.5).contains(s) {
            eprintln!("TURN {s:9.2}-{e:.2} sp{l}");
        }
    }
    let _model_path = format!("{models_dir}/pyannote-segmentation.onnx");
    eprintln!("--- cached full-meeting frame-mass track 12-17.5s and 32-38.5s ---");
    {
        let dump = |t0: f64, t1: f64| {
            let f0 = (t0 / FRAME_SHIFT) as usize;
            let f1 = ((t1 / FRAME_SHIFT) as usize).min(fm.frames.len());
            let mut row = String::new();
            let mut f = f0;
            while f < f1 {
                let t = f as f64 * FRAME_SHIFT;
                let m = &fm.frames[f];
                let speech = m.speaker[0] + m.speaker[1] + m.speaker[2];
                let best = if m.speaker[0] > m.speaker[1] && m.speaker[0] > m.speaker[2] {
                    0
                } else if m.speaker[1] > m.speaker[2] {
                    1
                } else {
                    2
                };
                if speech > 0.5 {
                    row.push_str(&format!("{:.1}:sp{}({:.2}) ", t, best, speech));
                } else {
                    row.push_str(&format!("{:.1}:sil({:.2}) ", t, m.silence));
                }
                f += 30; // ~0.5s steps
            }
            eprintln!("MASS {row}");
        };
        dump(12.0, 17.5);
        dump(32.0, 38.5);
    }
    eprintln!("--- per-window slot tracks ([w+2,w+8) mid-window frames) ---");
    for (ws, track) in &fm.window_label_tracks {
        if !(*ws >= 10.0 && *ws <= 16.5) && !(*ws >= 30.0 && *ws <= 36.5) {
            continue;
        }
        let mut row = format!("VOTE w={ws:>5.1} ");
        for k in 2..track.len().min(8) {
            let t = ws + k as f64 * FRAME_SHIFT;
            row.push_str(&format!("{:.1}:s{} ", t, track[k]));
        }
        eprintln!("{row}");
    }
    eprintln!("--- borrow distances at the target gap (midpoint rule, i64 ms) ---");
    for (name, a, b) in
        [("gate mega-row", 14.78, GAP_END), ("db fine row", 14.78, 15.84)]
    {
        let mid = ((a + b) / 2.0 * 1000.0) as i64;
        let w = borrow_winner(&pre_turns, mid);
        eprintln!(
            "BORROW {name}: span [{a:.2},{b:.2}] mid {mid}ms -> {:?}",
            w.map(|(l, d)| format!("sp{l} @ {d}ms"))
        );
    }

    // ---- 1.2 voiced-onset rehearsal inside the gap ----
    eprintln!("\n=== 1.2 onset rehearsal 14.5-16.5s ===");
    {
        let i0 = (14.5 * SAMPLE_RATE as f64) as usize;
        let hop = SAMPLE_RATE / 50;
        let mut row = String::new();
        let mut j = i0;
        while j + hop <= ((16.5 * SAMPLE_RATE as f64) as usize).min(samples.len()) {
            let t = j as f64 / SAMPLE_RATE as f64;
            let rms = (samples[j..j + hop].iter().map(|v| v * v).sum::<f32>() / hop as f32).sqrt();
            let db = 20.0 * rms.max(1e-10).log10();
            if t >= 14.5 && t <= 16.5 && ((t * 50.0).round() as i64) % 4 == 0 {
                row.push_str(&format!("{:.1}:{:.0} ", t, db));
            }
            j += hop;
        }
        eprintln!("ENERGY {row}");
    }
    let (onset, baseline_db, peak_db) = detect_onset(&samples, 14.78, GAP_END);
    eprintln!(
        "ONSET gap[14.78,16.12]: detected {:?} (baseline {:.1} dBFS, peak {:.1} dBFS) — ear says ≈15.8",
        onset, baseline_db, peak_db
    );

    // ---- 1.3 identity votes ----
    eprintln!("\n=== 1.3 identity votes vs final centroids ===");
    let embed = |a: f64, b: f64| -> Option<Vec<f32>> {
        let i0 = (a * SAMPLE_RATE as f64) as usize;
        let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
        if i1 <= i0 {
            return None;
        }
        extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)
    };
    let vote = |name: &str, e: Option<Vec<f32>>| match e {
        None => eprintln!("VOTE {name:<34} (no embedding — silence gate)"),
        Some(v) => {
            let mut sims: Vec<(u32, f32)> = cids
                .iter()
                .map(|c| (*c, cosine(&out.centroids[c], &v)))
                .collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            let margin = sims.first().zip(sims.get(1)).map(|(f, s)| f.1 - s.1);
            let list = sims
                .iter()
                .map(|(c, s)| format!("sp{c}={s:.3}"))
                .collect::<Vec<_>>()
                .join(" ");
            eprintln!(
                "VOTE {name:<34} best sp{} margin {:?} decided={} [{list}]",
                sims[0].0,
                margin.map(|m| format!("{m:.3}")),
                margin.map_or(false, |m| m >= RESCUE_MARGIN)
            );
        }
    };
    vote("raw gate window [14.78,16.12]", embed(14.78, GAP_END));
    vote("raw db window [14.78,15.84]", embed(14.78, 15.84));
    vote("padded gate [14.58,16.32]", embed(14.58, 16.32));
    vote("padded db [14.58,16.04]", embed(14.58, 16.04));
    if let Some(o) = onset {
        vote("voiced [onset,16.12]", embed(o, GAP_END));
        vote("voiced ear [15.8,16.12]", embed(15.8, GAP_END));
    } else {
        vote("voiced ear [15.8,16.12]", embed(15.8, GAP_END));
    }
    // Voice anchors as sanity: Cynthia's Okay row 16.2-20.18 is engine sp1.
    vote("sanity cynthia [16.3,17.5]", embed(16.3, 17.5));
    vote("sanity carlos [13.6,14.7]", embed(13.6, 14.7));

    // ---- replica of the engine's piece pipeline (for 1.6) ----
    let labels = local_labels(&fm.frames, SPEECH_GATE);
    let runs = speech_runs(&fm.frames, FRAME_SHIFT);
    let pieces = derive_pieces(
        &labels,
        &runs,
        &fm.window_label_tracks,
        FRAME_SHIFT,
        MODE_FILTER_RADIUS_FRAMES,
    );
    let kept_all = drop_textless(&pieces, &gate_spans, TEXT_SKEW_TOLERANCE_SECS);
    let kept: &[app_lib::audio::speaker::run_assembly::PieceSpan] = if kept_all.len() > PIECE_CAP {
        &kept_all[..PIECE_CAP]
    } else {
        &kept_all
    };
    eprintln!(
        "\nPROBE: replica pieces kept {}/{} (drop_textless + cap)",
        kept.len(),
        pieces.len()
    );
    let sr = SAMPLE_RATE as f64;
    let mut embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(kept.len());
    for p in kept {
        let dur = p.end_secs - p.start_secs;
        let (oa, ob) = embed_slice(dur);
        let i0 = ((p.start_secs + oa) * sr) as usize;
        let i1 = (((p.start_secs + ob) * sr) as usize).min(samples.len());
        embeddings.push(if i1 > i0 {
            extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)
        } else {
            None
        });
    }
    let labeled_idx: Vec<usize> = (0..kept.len())
        .filter(|&i| {
            embeddings[i].is_some() && kept[i].end_secs - kept[i].start_secs >= EMBED_FLOOR_SECS
        })
        .collect();
    let labeled_embs: Vec<Vec<f32>> = labeled_idx
        .iter()
        .map(|&i| embeddings[i].clone().expect("labeled emb"))
        .collect();
    let mut resolved: Vec<Option<(usize, f32, bool)>> = vec![None; kept.len()];
    let mut used: Vec<Vec<f32>> = Vec::new();
    if !labeled_embs.is_empty() {
        let refs: Vec<&Vec<f32>> = references
            .iter()
            .take(MEETING_CAP.max(1) - 1)
            .map(|(_, e)| e)
            .collect();
        let piece_cap = (MEETING_CAP - refs.len()).max(1);
        let (mut assign, mut centroids) = cluster_pieces(&labeled_embs, MERGE_THRESHOLD);
        merge_to_cap(&mut assign, &mut centroids, &labeled_embs, piece_cap);
        for e in &refs {
            centroids.push((*e).clone());
        }
        refine_loop(&mut assign, &mut centroids, &labeled_embs, 10);
        loop {
            let mut merge: Option<(usize, usize)> = None;
            for i in 0..centroids.len() {
                for j in (i + 1)..centroids.len() {
                    if cosine(&centroids[i], &centroids[j]) >= 0.85 {
                        merge = Some((j, i));
                        break;
                    }
                }
                if merge.is_some() {
                    break;
                }
            }
            let Some((victim, keeper)) = merge else { break };
            for a in assign.iter_mut() {
                if *a == victim {
                    *a = keeper;
                } else if *a > victim {
                    *a -= 1;
                }
            }
            centroids.remove(victim);
            let dim = centroids[0].len();
            let mut sums = vec![0.0f32; centroids.len() * dim];
            let mut counts = vec![0usize; centroids.len()];
            for (k, ci) in assign.iter().enumerate() {
                counts[*ci] += 1;
                for (d, v) in labeled_embs[k].iter().enumerate() {
                    sums[*ci * dim + d] += v;
                }
            }
            for (ci, c) in centroids.iter_mut().enumerate() {
                if counts[ci] > 0 {
                    for d in 0..dim {
                        c[d] = sums[ci * dim + d] / counts[ci] as f32;
                    }
                }
            }
        }
        let mut remap = std::collections::HashMap::new();
        for &c in &assign {
            let n = remap.len();
            remap.entry(c).or_insert(n);
        }
        let refined: Vec<usize> = assign.iter().map(|c| remap[c]).collect();
        // dedup centroids converged onto the same voice (first-seen order),
        // relabeling `refined` through the pruned set (phantom invariant)
        let mut dedup: Vec<Vec<f32>> = Vec::new();
        let mut idx_map = std::collections::HashMap::new();
        for c in refined.iter() {
            if let std::collections::hash_map::Entry::Vacant(v) = idx_map.entry(*c) {
                v.insert(dedup.len());
                dedup.push(centroids[*c].clone());
            }
        }
        let refined: Vec<usize> = refined.iter().map(|c| idx_map[c]).collect();
        used = dedup;
        let lab_of: std::collections::HashMap<usize, usize> = labeled_idx
            .iter()
            .enumerate()
            .map(|(k, &i)| (i, k))
            .collect();
        let best_centroid = |e: &[f32]| -> Option<(usize, f32)> {
            let mut sims: Vec<(usize, f32)> =
                used.iter().enumerate().map(|(ci, c)| (ci, cosine(c, e))).collect();
            sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            sims.first().and_then(|s| {
                sims.get(1).map(|(_, second)| (s.0, s.1 - second))
            })
        };
        for &i in &labeled_idx {
            let k = lab_of[&i];
            resolved[i] = Some((refined[k], margin_to_centroids(&labeled_embs[k], &used), false));
        }
        // Pass 1 + Pass 2 (faithful copy of run_engine.rs:243-296)
        for (i, piece) in kept.iter().enumerate() {
            if resolved[i].is_some() {
                continue;
            }
            let dur = piece.end_secs - piece.start_secs;
            if dur < EMBED_FLOOR_SECS {
                if let Some(e) = &embeddings[i] {
                    if let Some((c, m)) = best_centroid(e) {
                        if m >= AMBIGUITY_MARGIN && dur >= PROMOTION_FLOOR_SECS {
                            resolved[i] = Some((c, m, true));
                        }
                    }
                }
            }
        }
        for (i, piece) in kept.iter().enumerate() {
            if resolved[i].is_some() {
                continue;
            }
            let dur = piece.end_secs - piece.start_secs;
            if dur >= EMBED_FLOOR_SECS {
                continue;
            }
            let own = embeddings[i].as_ref().and_then(|e| best_centroid(e));
            let prev_l = (0..i).rev().find_map(|j| resolved[j].map(|(c, _, _)| c));
            let next_l = ((i + 1)..kept.len()).find_map(|j| resolved[j].map(|(c, _, _)| c));
            let neighbors_agree = matches!((prev_l, next_l), (Some(p), Some(n)) if p == n);
            let joins_neighbors = neighbors_agree
                && match (own, prev_l) {
                    (Some((c, m)), Some(p)) => c == p && m >= AMBIGUITY_MARGIN,
                    _ => false,
                };
            if dur >= PROMOTION_FLOOR_SECS && !joins_neighbors {
                let label = own.map(|(c, _)| c).or(prev_l).unwrap_or(0);
                resolved[i] = Some((label, 0.0, true));
            } else if neighbors_agree {
                resolved[i] = Some((prev_l.expect("checked"), AMBIGUITY_MARGIN, false));
            } else if matches!((prev_l, next_l), (Some(_), Some(_))) {
                let label = own.map(|(c, _)| c).or(prev_l).unwrap_or(0);
                resolved[i] = Some((label, 0.0, true));
            }
        }
    }
    let mut piece_ins: Vec<PieceIn> = Vec::with_capacity(kept.len());
    for (i, p) in kept.iter().enumerate() {
        let dur = p.end_secs - p.start_secs;
        match resolved[i] {
            Some((c, m, promoted)) => piece_ins.push(PieceIn {
                start_secs: p.start_secs,
                dur_secs: dur,
                cluster: Some(c),
                margin: Some(m),
                promoted_subfloor: promoted,
            }),
            None => piece_ins.push(PieceIn {
                start_secs: p.start_secs,
                dur_secs: dur,
                cluster: None,
                margin: None,
                promoted_subfloor: false,
            }),
        }
    }
    let replica_pre = resolve_turns(&piece_ins);
    let engine_pre: Vec<(u64, u64, i32)> = out
        .turns
        .iter()
        .map(|t| {
            (
                (t.start_seconds * 100.0).round() as u64,
                (t.end_seconds * 100.0).round() as u64,
                t.speaker_id as i32,
            )
        })
        .collect();
    let replica_tuples: Vec<(u64, u64, i32)> = replica_pre
        .iter()
        .map(|t| {
            (
                (t.start_secs * 100.0).round() as u64,
                (t.end_secs * 100.0).round() as u64,
                t.cluster as i32,
            )
        })
        .collect();
    let fidelity = engine_pre == replica_tuples;
    eprintln!(
        "PROBE: replica fidelity vs engine: {} (engine {} turns, replica {} turns)",
        if fidelity { "MATCH" } else { "DRIFT" },
        engine_pre.len(),
        replica_tuples.len()
    );

    // ---- 1.4 whole-meeting candidate scan ----
    eprintln!("\n=== 1.4 whole-meeting candidate scan (DB spans = production engine) ===");
    // fixture entries for disturbance mapping
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string("tests/fixtures/ear_truth_cde5c264.json").expect("fixture"),
    )
    .expect("parse fixture");
    let n_frames = fm.frames.len();
    let mut gaps: Vec<(f64, f64)> = Vec::new();
    if let Some(first) = runs.first() {
        let a = 0.0;
        let b = first.start_frame as f64 * FRAME_SHIFT;
        if b - a > 0.0 {
            gaps.push((a, b));
        }
    }
    for w in runs.windows(2) {
        let a = w[0].end_frame as f64 * FRAME_SHIFT;
        let b = w[1].start_frame as f64 * FRAME_SHIFT;
        if b - a > 0.0 {
            gaps.push((a, b));
        }
    }
    if let Some(last) = runs.last() {
        let a = last.end_frame as f64 * FRAME_SHIFT;
        let b = n_frames as f64 * FRAME_SHIFT;
        if b - a > 0.0 {
            gaps.push((a, b));
        }
    }
    let union_span = |spans: &[(f64, f64)], a: f64, b: f64| -> Option<(f64, f64)> {
        let mut lo = f64::MAX;
        let mut hi = f64::MIN;
        let mut any = false;
        for (s, e) in spans {
            let lo_s = (*s).max(a);
            let hi_s = (*e).min(b);
            if hi_s > lo_s {
                any = true;
                lo = lo.min(lo_s);
                hi = hi.max(hi_s);
            }
        }
        if any {
            Some((lo, hi.min(b)))
        } else {
            None
        }
    };
    let mut candidates = 0usize;
    for (ga, gb) in &gaps {
        let (fla, _fle, flc) = match pre_turns
            .iter()
            .filter(|(_, e, _)| *e <= *ga + 1e-9)
            .last()
        {
            Some(t) => *t,
            None => continue, // meeting-edge: no left flank -> not a candidate
        };
        let rt = match pre_turns.iter().find(|(s, _, _)| *s >= *gb - 1e-9) {
            Some(t) => *t,
            None => continue,
        };
        let (la, lc, ra, rc) = (fla, flc, rt.0, rt.2);
        let interior = lc == rc;
        let dist_turns = format!("sp{lc}[{la:.2}..]..[..{ra:.2}]sp{rc}");
        let db_span = union_span(&db_spans, *ga, *gb);
        let gate_span = union_span(&gate_spans, *ga, *gb);
        let (Some((sa, sb)), _src) = (match (db_span, gate_span) {
            (Some(d), _) => (Some((d.0, d.1)), "db"),
            (None, Some(g)) => (Some((g.0, g.1)), "gate"),
            _ => (None, "-"),
        }) else {
            // no text overlap -> true-silence control material
            continue;
        };
        let raw_dur = sb - sa;
        let mut reason = String::new();
        let mut ok = true;
        if interior {
            reason.push_str("interior ");
            ok = false;
        }
        if raw_dur < RAW_FLOOR_SECS {
            reason.push_str(&format!("floor({raw_dur:.2}<0.8) "));
            ok = false;
        }
        if ok {
            let mid = ((sa + sb) / 2.0 * 1000.0) as i64;
            let winner = borrow_winner(&pre_turns, mid);
            match raw_embed_and_vote(&extractor, &samples, sa, sb, &used) {
                Some((best, margin)) => {
                    if margin < RESCUE_MARGIN {
                        reason.push_str(&format!("margin({margin:.3}) "));
                        ok = false;
                    }
                    let flank_label = winner.map(|(l, _)| l);
                    if ok && flank_label == Some(best as u32) {
                        reason.push_str("best==winner ");
                        ok = false;
                    }
                    if ok {
                        match detect_onset(&samples, sa, sb) {
                            (Some(on), _, _) => {
                                candidates += 1;
                                eprintln!(
                                    "CANDIDATE gap[{ga:.2},{gb:.2}] {dist_turns} span[{sa:.2},{sb:.2}] dur {raw_dur:.2} best sp{best} margin {margin:.3} onset {on:.2} winner {:?}",
                                    flank_label.map(|l| format!("sp{l}"))
                                );
                            }
                            _ => {
                                reason.push_str("no-onset ");
                                ok = false;
                            }
                        }
                    }
                }
                None => {
                    reason.push_str("no-embedding ");
                    ok = false;
                }
            }
        }
        if !ok {
            eprintln!(
                "gap[{ga:.2},{gb:.2}] span[{sa:.2},{sb:.2}] {dist_turns} ABSTAIN: {reason}"
            );
        }
        // fixture disturbance mapping (span OR pin window intersect influence zone)
        let infl = (ga - (gb - ga), gb + (gb - ga));
        for e in &fixture.entries {
            let span_hit = e.start_s <= infl.1 && e.end_s >= infl.0;
            let pin_hit = e
                .change_at_s
                .map(|p| {
                    let tol = e.tolerance_s.unwrap_or(0.5);
                    p + tol >= infl.0 && p - tol <= infl.1
                })
                .unwrap_or(false);
            if span_hit || pin_hit {
                eprintln!(
                    "  -> touches {} [{}..{}] pin {:?}",
                    e.id, e.start_s, e.end_s, e.change_at_s
                );
            }
        }
    }
    eprintln!("PROBE: whole-meeting candidates = {candidates}");

    // ---- 1.5 controls: longest true-silence gaps ----
    eprintln!("\n=== 1.5 controls: longest no-text gaps under the same gates ===");
    let mut silence_gaps: Vec<(f64, f64)> = gaps
        .iter()
        .filter(|(a, b)| union_span(&db_spans, *a, *b).is_none())
        .cloned()
        .collect();
    silence_gaps.sort_by(|a, b| (b.1 - b.0).partial_cmp(&(a.1 - a.0)).unwrap());
    for (ga, gb) in silence_gaps.iter().take(8) {
        let mid = ((ga + gb) / 2.0 * 1000.0) as i64;
        let win_a = ga.max(gb - 12.0);
        let (best, margin) =
            raw_embed_and_vote(&extractor, &samples, win_a, *gb, &used).unwrap_or((0, -1.0));
        let (on, base, peak) = detect_onset(&samples, win_a, *gb);
        eprintln!(
            "CONTROL gap[{ga:.2},{gb:.2}] dur {:.2} best sp{best} margin {margin:.3} onset {on:?} (base {base:.0} peak {peak:.0}) winner {:?}",
            gb - ga,
            borrow_winner(&pre_turns, mid).map(|(l, d)| format!("sp{l}@{d}"))
        );
    }

    // ---- 1.6 post-splice predicate rehearsal ----
    eprintln!("\n=== 1.6 post-splice rehearsal (splice AT ONSET) ===");
    let onset = onset.unwrap_or(15.8);
    let mut spliced = piece_ins.clone();
    let insert_at = spliced
        .iter()
        .position(|p| p.start_secs >= onset)
        .unwrap_or(spliced.len());
    spliced.insert(
        insert_at,
        PieceIn {
            start_secs: onset,
            dur_secs: GAP_END - onset,
            cluster: Some(1), // Cynthia's cluster on this meeting (engine sp1); verified by 1.3 votes
            margin: Some(RESCUE_MARGIN),
            promoted_subfloor: true,
        },
    );
    let post = resolve_turns(&spliced);
    for t in post.iter().filter(|t| t.start_secs < 21.0 && t.end_secs > 9.0) {
        eprintln!(
            "POST TURN {:.2}-{:.2} c{}{}",
            t.start_secs,
            t.end_secs,
            t.cluster,
            if t.low_confidence { " lowconf" } else { "" }
        );
    }
    let post_turns: Vec<(f64, f64, u32)> = post
        .iter()
        .map(|t| (t.start_secs, t.end_secs, t.cluster as u32))
        .collect();
    let changes: Vec<f64> = post_turns
        .windows(2)
        .filter(|w| w[0].2 != w[1].2)
        .map(|w| w[1].0)
        .collect();
    let in_s2b: Vec<f64> = changes
        .iter()
        .filter(|t| **t >= 15.0 && **t <= 16.8)
        .cloned()
        .collect();
    let s2b = in_s2b.len() == 1 && (in_s2b[0] - 15.8).abs() <= 0.75;
    let single = |a: f64, b: f64| -> bool {
        let mut labels: Vec<u32> = Vec::new();
        for (s, e, l) in &post_turns {
            if e.min(b) - s.max(a) > 0.25 {
                labels.push(*l);
            }
        }
        !labels.is_empty() && labels.iter().all(|l| *l == labels[0])
    };
    eprintln!(
        "REHEARSAL S2b (change@15.8 in [15.0,16.8] tol 0.75): {} (changes {in_s2b:?})",
        if s2b { "PASS" } else { "FAIL" }
    );
    eprintln!(
        "REHEARSAL S1 [9.38,11.8] single_voice: {}",
        if single(9.38, 11.8) { "PASS" } else { "FAIL" }
    );
    eprintln!(
        "REHEARSAL S3 [15.5,20.83] single_voice: {}",
        if single(15.5, 20.83) { "PASS" } else { "FAIL" }
    );
    eprintln!("PROBE: done");
}

/// Embed a span and vote against `used` centroids; returns (best, margin).
fn raw_embed_and_vote(
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    a: f64,
    b: f64,
    used: &[Vec<f32>],
) -> Option<(usize, f32)> {
    let i0 = (a * SAMPLE_RATE as f64) as usize;
    let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
    if i1 <= i0 {
        return None;
    }
    let e = extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)?;
    let mut sims: Vec<(usize, f32)> =
        used.iter().enumerate().map(|(ci, c)| (ci, cosine(c, &e))).collect();
    sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let best = sims.first()?.0;
    let margin = sims.first()?.1 - sims.get(1)?.1;
    Some((best, margin))
}
