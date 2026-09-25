//! Row-voice census (2026-09-22 ear failure: "constantly absorbing my
//! sentences into UserB's and vice versa").
//!
//! For EVERY persisted transcript row: slice its span into voiced sub-runs
//! (energy-gated), embed each sub-run with TitaNet, and vote against the
//! three meeting voices' fingerprints (UserA/UserB enrollment means +
//! the meeting-stamped UserC voiceprint). A row whose votes disagree with
//! its persisted badge — or split between two voices — is an absorption the
//! ear caught meeting-wide. This is the same trust the gap rescue already
//! carries into production (TitaNet hears voice where pyannote's turns are
//! too coarse); here it audits the render instead of filling silence.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --release --test row_voice_census_probe -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::nemo_extractor::NemoEmbeddingExtractor;
use app_lib::audio::speaker::run_assembly::cosine;
use sqlx::Row;

const AUDIO: &str =
    "Music/local-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const MODELS_DIR: &str = ".meetily-models";
const SAMPLE_RATE: usize = 16_000;
/// Decided vote: top-1 must beat top-2 by this margin (same bar the gap
/// rescue uses before it trusts a TitaNet verdict).
const VOTE_MARGIN: f32 = 0.05;
/// Minimum voiced sub-run worth an embedding.
const MIN_SUB_SECS: f64 = 0.3;
/// Long voiced runs are chunked so no embedding spans more than this.
const MAX_CHUNK_SECS: f64 = 3.0;
/// A row counts as mixed when the losing voice holds at least this share
/// of decided votes.
const MIXED_SHARE: f32 = 0.2;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

/// Energy-gated voiced sub-runs inside [a, b] (probe-local copy of the v3
/// gate: 20ms RMS hop, adaptive threshold baseline+10dB).
fn segment_voiced(samples: &[f32], a: f64, b: f64) -> Vec<(f64, f64)> {
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
        .filter(|(s, e)| e - s >= MIN_SUB_SECS)
        .collect()
}

fn load_samples() -> Vec<f32> {
    use sha2::{Digest, Sha256};
    let audio_path = format!("{}/{AUDIO}", home());
    let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
    let cache = dir.join("samples_16k.f32");
    let meta = dir.join("samples_16k.meta.json");
    let mut h = Sha256::new();
    let mut file = std::fs::File::open(&audio_path).expect("open source audio");
    std::io::copy(&mut file, &mut h).expect("hash source audio");
    let source_sha = format!("{:x}", h.finalize());
    if cache.exists() && meta.exists() {
        let meta_json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&meta).expect("read samples meta"))
                .expect("parse samples meta");
        match meta_json.get("audio_sha256").and_then(|v| v.as_str()) {
            Some(pinned) => assert_eq!(
                pinned, source_sha,
                "samples cache was built from different audio — delete samples_16k.f32"
            ),
            None => eprintln!(
                "PROBE: WARNING samples cache has NO audio provenance (legacy meta) — delete it to re-pin"
            ),
        }
        let bytes = std::fs::read(&cache).expect("read samples cache");
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        eprintln!(
            "PROBE: samples loaded from cache ({} = {:.1}s)",
            samples.len(),
            samples.len() as f64 / 16_000.0
        );
        return samples;
    }
    panic!("samples cache missing — run the ear gate once to build samples_16k.f32");
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Voice {
    UserA,
    UserB,
    UserC,
}

fn badge_to_voice(badge: &str) -> Option<Voice> {
    match badge {
        "UserA" => Some(Voice::UserA),
        "UserB" => Some(Voice::UserB),
        "Speaker 2" => Some(Voice::UserC),
        _ => None,
    }
}

fn voice_tag(v: Voice) -> &'static str {
    match v {
        Voice::UserA => "UA",
        Voice::UserB => "UB",
        Voice::UserC => "RI",
    }
}

fn l2normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test row_voice_census_probe -- --ignored --nocapture"]
async fn census_row_voices_vs_badges() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples = load_samples();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(format!("{}/{}", home(), DB_PATH)),
        )
        .await
        .expect("db connect");

    // Enrollment means for the named voices; UserC's voiceprint is the
    // meeting-stamped centroid under his auto speaker id (he was never
    // enrolled by name — the live run stamps one embedding per cluster).
    let mut means: Vec<(Voice, Vec<f32>)> = Vec::new();
    for (id, vec) in app_lib::database::repositories::speaker::SpeakerRepository::list_enrollment_refs(&pool)
        .await
        .expect("enrollment refs")
    {
        let v = match id.as_str() {
            "speaker-447b345b-e4d3-41d3-a971-32863dc0c5d4" => Some(Voice::UserA),
            "speaker-0a2ba31a-6f2c-4c13-9a64-05e0a24fed0a" => Some(Voice::UserB),
            _ => None,
        };
        if let Some(v) = v {
            means.push((v, vec));
        }
    }
    // UserC was never enrolled by name; his voiceprint is the
    // meeting-stamped centroid under the auto speaker id (one embedding per
    // cluster, stamped by the last live run). list_stamped_embeddings
    // excludes auto ids by design, so query it directly.
    let userC_id = format!("speaker-auto-{MEETING_ID}-2");
    let userC_rows = sqlx::query(
        "SELECT e.embedding AS embedding FROM speaker_embeddings e WHERE e.speaker_id = ?",
    )
    .bind(&userC_id)
    .fetch_all(&pool)
    .await
    .expect("userC stamped embedding");
    let userC_vecs: Vec<Vec<f32>> = userC_rows
        .iter()
        .map(|r| {
            let blob: Vec<u8> = r.get("embedding");
            app_lib::database::repositories::speaker::SpeakerRepository::deserialize_embedding(
                &blob,
            )
            .expect("deserialize userC embedding")
        })
        .collect();
    assert!(!userC_vecs.is_empty(), "no stamped embedding for {userC_id}");
    {
        let dim = userC_vecs[0].len();
        let mut mean = vec![0.0f32; dim];
        for v in &userC_vecs {
            for (m, x) in mean.iter_mut().zip(v.iter()) {
                *m += x;
            }
        }
        l2normalize(&mut mean);
        means.push((Voice::UserC, mean));
    }
    assert_eq!(means.len(), 3, "need all three voice fingerprints");
    means.sort_by_key(|(v, _)| *v as u8);

    let rows = sqlx::query(
        "SELECT audio_start_time, audio_end_time, COALESCE(speaker_label, speaker, '?') AS badge, transcript \
         FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("fetch rows");
    drop(pool);

    let extractor = NemoEmbeddingExtractor::new(&format!(
        "{}/{MODELS_DIR}/{}",
        home(),
        app_lib::audio::speaker::model_download::embedding_filename()
    ))
    .expect("embedding model");

    let mut stats: std::collections::BTreeMap<String, usize> = Default::default();
    let mut offender_lines: Vec<String> = Vec::new();
    for r in &rows {
        let s: f64 = r.get("audio_start_time");
        let e: f64 = r.get("audio_end_time");
        let badge: String = r.get("badge");
        let text: String = r.get("transcript");
        let badge_voice = match badge_to_voice(&badge) {
            Some(v) => v,
            None => {
                *stats.entry("badge-unmapped".into()).or_default() += 1;
                continue;
            }
        };
        let mut votes: std::collections::BTreeMap<Voice, usize> = Default::default();
        let mut abstain = 0usize;
        let mut timeline: Vec<(f64, f64, Voice)> = Vec::new();
        for (a, b) in segment_voiced(&samples, s, e) {
            let mut chunk = a;
            while chunk < b - 1e-9 {
                let end = (chunk + MAX_CHUNK_SECS).min(b);
                let i0 = (chunk * SAMPLE_RATE as f64) as usize;
                let i1 = ((end * SAMPLE_RATE as f64) as usize).min(samples.len());
                if let Some(emb_vec) = extractor.extract_embedding(&samples[i0..i1], SAMPLE_RATE as u32) {
                    let mut emb = emb_vec;
                    l2normalize(&mut emb);
                    let mut scored: Vec<(f32, Voice)> = means
                        .iter()
                        .map(|(v, m)| (cosine(m, &emb), *v))
                        .collect();
                    scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
                    if scored.len() >= 2 && scored[0].0 - scored[1].0 >= VOTE_MARGIN {
                        *votes.entry(scored[0].1).or_default() += 1;
                        timeline.push((chunk, end, scored[0].1));
                    } else {
                        abstain += 1;
                    }
                }
                chunk = end;
            }
        }
        let total: usize = votes.values().sum();
        if total == 0 {
            *stats.entry("no-decided-votes".into()).or_default() += 1;
            continue;
        }
        let mut ranked: Vec<(Voice, usize)> = votes.clone().into_iter().collect();
        ranked.sort_by(|x, y| y.1.cmp(&x.1).then(x.0.cmp(&y.0)));
        let majority = ranked[0].0;
        let loser_share = if ranked.len() > 1 {
            ranked[1].1 as f32 / total as f32
        } else {
            0.0
        };
        let kind = if ranked.len() > 1 && loser_share >= MIXED_SHARE {
            "MIXED"
        } else if majority != badge_voice {
            "DISAGREE"
        } else {
            "agree"
        };
        *stats.entry(kind.into()).or_default() += 1;
        if kind != "agree" {
            let tl = timeline
                .iter()
                .map(|(a, b, v)| format!("{}[{:.2}-{:.2}]", voice_tag(*v), a, b))
                .collect::<Vec<_>>()
                .join(" ");
            offender_lines.push(format!(
                "[{:8.2}-{:8.2}] badge={} votes={} abstain={} {} | {}",
                s,
                e,
                voice_tag(badge_voice),
                ranked
                    .iter()
                    .map(|(v, n)| format!("{}x{}", voice_tag(*v), n))
                    .collect::<Vec<_>>()
                    .join(","),
                abstain,
                kind,
                tl
            ));
            offender_lines.push(format!("        TEXT: {}", text));
        }
    }

    eprintln!("\n===== ROW-VOICE CENSUS ({} persisted rows) =====", rows.len());
    for (k, v) in &stats {
        eprintln!("  {k}: {v}");
    }
    eprintln!("\n===== OFFENDERS (mixed or badge-disagreeing rows) =====");
    for line in &offender_lines {
        eprintln!("{line}");
    }
}
