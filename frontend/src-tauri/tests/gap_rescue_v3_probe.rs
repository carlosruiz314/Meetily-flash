//! Gap-rescue probe iteration 2: the v3 sub-window candidate scan
//! (change `gap-speech-voice-attribution`, probe re-run after the 1.7 NO-GO).
//!
//! MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan --test gap_rescue_probe -- --ignored --nocapture gap_rescue_v3_scan
//!
//! v3 mechanism under test: energy-segment each text-bearing distinct-turn
//! gap's raw span into voiced sub-windows; embed and decide EACH sub-window
//! against the meeting's final centroids; attribute the transcript row to the
//! LAST decided sub-window (skew-aware: legacy rows skew early); splice exactly
//! that sub-window's span. Rehearsal is ANALYTIC at the turn level (the
//! piece-list replica drifted in iteration 1 — shed-merge not replicated); the
//! real engine path is verified by the gate after implementation.

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::pyannote_segmentation::{
    FrameMassesOutput, PyannoteSegmentation, FRAME_SHIFT,
};
use app_lib::audio::speaker::run_assembly::{cosine, speech_runs, PROMOTION_FLOOR_SECS};
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
const BORROW_CAP_MS: i64 = 3_000;
const RESCUE_MARGIN: f32 = 0.05;
const RAW_FLOOR_SECS: f64 = PROMOTION_FLOOR_SECS;
const MIN_SUB_SECS: f64 = 0.12;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

#[derive(Deserialize)]
struct Entry {
    id: String,
    kind: String,
    start_s: f64,
    end_s: f64,
    #[serde(default)]
    change_at_s: Option<f64>,
    #[serde(default)]
    tolerance_s: Option<f64>,
}

#[derive(Deserialize)]
struct Fixture {
    entries: Vec<Entry>,
}

fn dist_ms(p: i64, s: i64, e: i64) -> i64 {
    if p >= s && p < e {
        0
    } else if p < s {
        s - p
    } else {
        p - e
    }
}

fn borrow_winner(turns: &[(f64, f64, u32)], mid: i64) -> Option<(u32, i64)> {
    let mut best: Option<(u32, i64)> = None;
    for (s, e, l) in turns {
        let d = dist_ms(mid, (*s * 1000.0) as i64, (*e * 1000.0) as i64);
        if d > BORROW_CAP_MS {
            continue;
        }
        match best {
            Some((_, bd)) if d >= bd => {}
            _ => best = Some((*l, d)),
        }
    }
    best
}

fn segment_voiced(samples: &[f32], a: f64, b: f64, min_sub_secs: f64) -> Vec<(f64, f64)> {
    let hop = SAMPLE_RATE / 50;
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
        return Vec::new();
    }
    let mut sorted = dbs.clone();
    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let baseline = sorted[sorted.len() / 4];
    let thr = baseline + 10.0;
    let mut runs: Vec<(usize, usize)> = Vec::new();
    let mut k = 0usize;
    while k < dbs.len() {
        if dbs[k] >= thr {
            let start = k;
            while k < dbs.len() && dbs[k] >= thr {
                k += 1;
            }
            runs.push((start, k));
        } else {
            k += 1;
        }
    }
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for r in runs {
        match merged.last_mut() {
            Some(last) if r.0 - last.1 < 4 => last.1 = r.1,
            _ => merged.push(r),
        }
    }
    merged
        .iter()
        .map(|(s, e)| {
            (
                a + (*s * hop) as f64 / SAMPLE_RATE as f64,
                a + (*e * hop) as f64 / SAMPLE_RATE as f64,
            )
        })
        .filter(|(s, e)| e - s >= min_sub_secs)
        .collect()
}

fn raw_embed_and_vote(
    extractor: &NemoEmbeddingExtractor,
    samples: &[f32],
    a: f64,
    b: f64,
    centroids: &[(u32, Vec<f32>)],
) -> Option<(u32, f32)> {
    let i0 = (a * SAMPLE_RATE as f64) as usize;
    let i1 = ((b * SAMPLE_RATE as f64) as usize).min(samples.len());
    if i1 <= i0 {
        return None;
    }
    let e = extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32)?;
    let mut sims: Vec<(u32, f32)> =
        centroids.iter().map(|(id, c)| (*id, cosine(c, &e))).collect();
    sims.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let best = sims.first()?.0;
    let margin = sims.first()?.1 - sims.get(1)?.1;
    Some((best, margin))
}

fn union_span(spans: &[(f64, f64)], a: f64, b: f64) -> Option<(f64, f64)> {
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
        Some((lo, hi))
    } else {
        None
    }
}

fn eval_entry(turns: &[(f64, f64, u32)], e: &Entry) -> (bool, String) {
    match e.kind.as_str() {
        "single_voice" => {
            let mut labs: Vec<u32> = Vec::new();
            for (s, en, l) in turns {
                let ov = en.min(e.end_s) - s.max(e.start_s);
                if ov > 0.25 {
                    labs.push(*l);
                }
            }
            let ok = !labs.is_empty() && labs.iter().all(|l| *l == labs[0]);
            (ok, format!("labels {labs:?}"))
        }
        "voice_change_at" | "multi_voice" => {
            let changes: Vec<f64> = turns
                .windows(2)
                .filter(|w| w[0].2 != w[1].2 && w[1].0 >= e.start_s && w[1].0 <= e.end_s)
                .map(|w| w[1].0)
                .collect();
            match e.kind.as_str() {
                "multi_voice" => (!changes.is_empty(), format!("changes {changes:?}")),
                _ => {
                    let (Some(pin), Some(tol)) = (e.change_at_s, e.tolerance_s) else {
                        return (false, "missing pin".into());
                    };
                    (
                        changes.len() == 1 && (changes[0] - pin).abs() <= tol,
                        format!("changes {changes:?} vs {pin}±{tol}"),
                    )
                }
            }
        }
        _ => (true, "(kind not evaluated)".into()),
    }
}

#[tokio::test]
#[ignore = "live probe: MEETIFY_LIVE_DIAG=1 cargo test --release --features vulkan --test gap_rescue_probe -- --ignored --nocapture gap_rescue_v3_scan"]
async fn gap_rescue_v3_scan() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let audio_path = format!("{}/{}", home(), AUDIO);
    let decoded = app_lib::audio::decoder::decode_audio_file(std::path::Path::new(&audio_path))
        .expect("decode audio");
    let samples = decoded.to_whisper_format();

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
    let mut centroids: Vec<(u32, Vec<f32>)> = out
        .centroids
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    centroids.sort_by_key(|(k, _)| *k);
    eprintln!(
        "V3: pre-rescue {} turns, {} clusters, {} refs",
        pre_turns.len(),
        centroids.len(),
        references.len()
    );

    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/ear_truth_cde5c264.json"
        ))
        .expect("fixture"),
    )
    .expect("parse fixture");

    let runs = speech_runs(&fm.frames, FRAME_SHIFT);
    let n_frames = fm.frames.len();
    let mut gaps: Vec<(f64, f64)> = Vec::new();
    if let Some(first) = runs.first() {
        let b = first.start_frame as f64 * FRAME_SHIFT;
        if b > 0.0 {
            gaps.push((0.0, b));
        }
    }
    for w in runs.windows(2) {
        let a = w[0].end_frame as f64 * FRAME_SHIFT;
        let b = w[1].start_frame as f64 * FRAME_SHIFT;
        if b > a {
            gaps.push((a, b));
        }
    }
    if let Some(last) = runs.last() {
        let a = last.end_frame as f64 * FRAME_SHIFT;
        let b = n_frames as f64 * FRAME_SHIFT;
        if b > a {
            gaps.push((a, b));
        }
    }

    let mut rehearsal = pre_turns.clone();
    let mut v3_candidates = 0usize;

    for (ga, gb) in &gaps {
        let Some(&(fla, _fle, flc)) =
            pre_turns.iter().filter(|(_, e, _)| *e <= *ga + 1e-9).last()
        else {
            continue;
        };
        let Some(&(ra, _reb, rc)) = pre_turns.iter().find(|(s, _, _)| *s >= *gb - 1e-9) else {
            continue;
        };
        if flc == rc {
            continue; // interior
        }
        let Some((sa, sb)) = union_span(&db_spans, *ga, *gb).or(union_span(&gate_spans, *ga, *gb))
        else {
            continue; // true silence: control class, never a candidate
        };
        if sb - sa < RAW_FLOOR_SECS {
            continue;
        }
        let subs = segment_voiced(&samples, sa, sb, MIN_SUB_SECS);
        let mut decided: Vec<(f64, f64, u32, f32)> = Vec::new();
        for (ssa, sse) in &subs {
            if let Some((best, margin)) =
                raw_embed_and_vote(&extractor, &samples, *ssa, *sse, &centroids)
            {
                if margin >= RESCUE_MARGIN {
                    decided.push((*ssa, *sse, best, margin));
                }
            }
        }
        let Some(&(ssa, sse, cluster, margin)) = decided.last() else {
            eprintln!(
                "v3 gap[{ga:.2},{gb:.2}] span[{sa:.2},{sb:.2}] ABSTAIN: no decided sub-window ({} voiced)",
                subs.len()
            );
            continue;
        };
        // same-voice continuation adjacent to its flank -> no piece
        if cluster == flc && (ssa - *ga).abs() < 0.6 {
            eprintln!(
                "v3 gap[{ga:.2},{gb:.2}] sub[{ssa:.2},{sse:.2}] sp{cluster} = left-flank continuation (no piece)"
            );
            continue;
        }
        let mid = ((sa + sb) / 2.0 * 1000.0) as i64;
        let winner = borrow_winner(&pre_turns, mid);
        if winner.map(|(l, _)| l) == Some(cluster) {
            eprintln!(
                "v3 gap[{ga:.2},{gb:.2}] sub[{ssa:.2},{sse:.2}] sp{cluster} NO-OP: borrow winner already sp{cluster}"
            );
            continue;
        }
        eprintln!(
            "v3-CANDIDATE gap[{ga:.2},{gb:.2}] {fla:.2}:sp{flc} .. sp{rc}:{ra:.2} sub[{ssa:.2},{sse:.2}] sp{cluster} margin {margin:.3} winner {:?} ({} voiced, {} decided)",
            winner.map(|(l, d)| format!("sp{l}@{d}")),
            subs.len(),
            decided.len()
        );
        v3_candidates += 1;
        // analytic rehearsal: apply the doctrine at turn level, time order
        if cluster == rc {
            for t in rehearsal.iter_mut() {
                if t.0 == ra && t.2 == rc {
                    t.0 = ssa;
                    break;
                }
            }
        } else if cluster == flc {
            for t in rehearsal.iter_mut() {
                if t.1 == fla && t.2 == flc {
                    t.1 = sse;
                    break;
                }
            }
        } else {
            rehearsal.push((ssa, sse, cluster));
            rehearsal.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        }
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
    eprintln!("V3: whole-meeting candidates = {v3_candidates}");

    eprintln!("\n=== analytic rehearsal over all fixture entries (post-v3 turn set) ===");
    for e in &fixture.entries {
        let (ok, detail) = eval_entry(&rehearsal, e);
        eprintln!(
            "REHEARSE {} [{}..{}] {} — {}",
            e.id,
            e.start_s,
            e.end_s,
            if ok { "PASS" } else { "FAIL" },
            detail
        );
    }
    eprintln!("V3: done");
}
