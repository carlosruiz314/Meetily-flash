//! Re-transcribe the transcript_sources rows whose token_timestamps are
//! empty (pre-token-population era rows): decode their audio span with the
//! production whisper model and write back text + token JSON, so the render
//! gets real word walls everywhere. The 2026-09-22 census traced the
//! meeting-wide label errors to these rows' proportional walls.
//!
//! Run: MEETIFY_LIVE_DIAG=1 cargo test --release --test retranscribe_empty_token_rows -- --ignored --nocapture

#![cfg(test)]

use app_lib::audio::speaker::token_timestamps::is_eot_marker;
use serde::Serialize;
use sqlx::Row;
use whisper_rs::WhisperContextParameters;

const AUDIO: &str =
    "Music/local-recordings/Meeting 2026-06-22_16-04-01_2026-06-22_14-04/audio.mp4";
const DB_PATH: &str = "AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite";
const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const MODEL: &str = "AppData/Roaming/com.meetily.ai/models/ggml-large-v3-turbo-q5_0.bin";
const SAMPLE_RATE: usize = 16_000;

fn home() -> String {
    std::env::var("USERPROFILE").unwrap()
}

#[derive(Serialize)]
struct Tok {
    word: String,
    start_ms: i64,
    end_ms: i64,
}

fn load_samples() -> Vec<f32> {
    use sha2::{Digest, Sha256};
    let audio_path = format!("{}/{AUDIO}", home());
    let dir = std::path::Path::new(&audio_path).parent().unwrap().to_path_buf();
    let mut h = Sha256::new();
    let mut file = std::fs::File::open(&audio_path).expect("open source audio");
    std::io::copy(&mut file, &mut h).expect("hash source audio");
    let source_sha = format!("{:x}", h.finalize());
    let bytes = std::fs::read(dir.join("samples_16k.f32")).expect("samples cache");
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("samples_16k.meta.json")).expect("samples meta"),
    )
    .expect("parse meta");
    if let Some(pinned) = meta.get("audio_sha256").and_then(|v| v.as_str()) {
        assert_eq!(pinned, source_sha, "samples cache provenance mismatch");
    }
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// The aligner's validity walk (piece merge must reconstruct the row text
/// exactly) — the probe refuses to write rows that would still fall back.
fn merge_validates(text: &str, toks: &[Tok]) -> bool {
    let mut i = 0usize;
    for w in text.split_whitespace() {
        let want: Vec<char> = w.chars().collect();
        let mut acc: Vec<char> = Vec::new();
        while acc != want {
            if acc.len() >= want.len() || i >= toks.len() {
                return false;
            }
            acc.extend(toks[i].word.trim().chars());
            i += 1;
        }
    }
    i == toks.len()
}

#[tokio::test]
#[ignore = "offline probe: MEETIFY_LIVE_DIAG=1 cargo test --release --test retranscribe_empty_token_rows -- --ignored --nocapture"]
async fn retranscribe_rows_without_tokens() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let samples = load_samples();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(format!("{}/{}", home(), DB_PATH)),
        )
        .await
        .expect("db connect (read-write)");
    let rows = sqlx::query(
        "SELECT id, audio_start_time, audio_end_time FROM transcript_sources \
         WHERE meeting_id = ? AND (token_timestamps IS NULL OR token_timestamps = '') \
         ORDER BY audio_start_time",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .expect("fetch empty-token rows");
    eprintln!("RETRANS: {} rows without tokens", rows.len());
    if rows.is_empty() {
        return;
    }

    let model_path = format!("{}/{MODEL}", home());
    let ctx = whisper_rs::WhisperContext::new_with_params(
        std::path::Path::new(&model_path),
        WhisperContextParameters::default(),
    )
    .expect("load whisper model");

    for r in &rows {
        let id: String = r.get("id");
        let s: f64 = r.get("audio_start_time");
        let e: f64 = r.get("audio_end_time");
        let i0 = (s * SAMPLE_RATE as f64) as usize;
        let i1 = ((e * SAMPLE_RATE as f64) as usize).min(samples.len());
        let audio = &samples[i0..i1];
        let offset_ms = (s * 1000.0) as i64;

        // The production strict posture (apply_strict_params + the
        // strict-segments override): token timestamps ON is what makes
        // token_data().t0/t1 real; without it every piece decodes with
        // degenerate times and the text walk misbehaves.
        let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy {
            best_of: 5,
        });
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_no_timestamps(false);
        params.set_token_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_temperature(0.0);
        params.set_max_initial_ts(1.0);
        params.set_entropy_thold(2.4);
        params.set_logprob_thold(-1.0);
        params.set_no_speech_thold(0.55);

        let mut state = ctx.create_state().expect("whisper state");
        state.full(params, audio).expect("whisper full");

        let n = state.full_n_segments();
        let mut texts: Vec<String> = Vec::new();
        let mut toks: Vec<Tok> = Vec::new();
        for i in 0..n {
            let Some(segment) = state.get_segment(i) else {
                continue;
            };
            let Ok(raw) = segment.to_str_lossy() else {
                continue;
            };
            let text = app_lib::audio::speaker::token_timestamps::strip_eot_markers(&raw);
            if text.trim().is_empty() {
                continue;
            }
            texts.push(text.trim().to_string());
            for t in 0..segment.n_tokens() {
                let Some(token) = segment.get_token(t) else {
                    continue;
                };
                let Ok(word) = token.to_str_lossy() else {
                    continue;
                };
                let word = word.trim().to_string();
                // Special tokens render as "[_BEG_]" / "[_TT_71]" with
                // positive ids — filter by rendering, not just id sign.
                if word.is_empty()
                    || is_eot_marker(&word)
                    || word.starts_with("[_")
                    || token.token_id() < 0
                {
                    continue;
                }
                let data = token.token_data();
                let start_ms = data.t0 as i64 * 10 + offset_ms;
                let end_ms = data.t1 as i64 * 10 + offset_ms;
                if end_ms < start_ms {
                    continue;
                }
                toks.push(Tok { word, start_ms, end_ms });
            }
        }
        let text = texts.join(" ");
        if text.is_empty() || !merge_validates(&text, &toks) {
            eprintln!(
                "RETRANS SKIP [{s:.2},{e:.2}] — decode unusable (text len {} tokens {})",
                text.len(),
                toks.len()
            );
            if std::env::var("RETRANS_DEBUG").is_ok() {
                eprintln!("RETRANS DEBUG text: {text:?}");
                let raw_pieces: Vec<String> = {
                    let mut v = Vec::new();
                    for i in 0..n {
                        if let Some(seg) = state.get_segment(i) {
                            for t in 0..seg.n_tokens() {
                                if let Some(tok) = seg.get_token(t) {
                                    v.push(tok.to_str_lossy().map(|x| x.to_string()).unwrap_or_default());
                                }
                            }
                        }
                    }
                    v
                };
                eprintln!("RETRANS DEBUG first 30 pieces: {:?}", &raw_pieces[..raw_pieces.len().min(30)]);
            }
            continue;
        }
        let token_json = serde_json::to_string(&toks).expect("serialize tokens");
        sqlx::query(
            "UPDATE transcript_sources SET transcript = ?, token_timestamps = ? WHERE id = ?",
        )
        .bind(&text)
        .bind(&token_json)
        .bind(&id)
        .execute(&pool)
        .await
        .expect("update source row");
        eprintln!(
            "RETRANS [{s:.2},{e:.2}] wrote {} chars / {} tokens: {}",
            text.len(),
            toks.len(),
            &text[..text.len().min(70)]
        );
    }
    let left: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transcript_sources \
         WHERE meeting_id = ? AND (token_timestamps IS NULL OR token_timestamps = '')",
    )
    .bind(MEETING_ID)
    .fetch_one(&pool)
    .await
    .expect("count leftover");
    eprintln!("RETRANS: rows still without tokens: {}", left.0);
}
