//! Guard-validation probe (overlap-repair-net-worse-guard task 1.3): for a
//! named window shape + seed span, run separation + both stream decodes,
//! assemble the repair render (the two stream texts), scope the replaced
//! mixture rows' tokens OUTSIDE the seed span into per-badge conservation
//! groups (the same scoping the guard will use), and compute the per-badge
//! alignment matrix. Token-only terminal output (verbatim ONLY under
//! MEETIFY_RENDER_PRINT); the recorded numbers settle the offline
//! stop-point (task 1.4/1.5).
//!
//! Env:
//!   MEETIFY_LIVE_DIAG=1          (gate for any model load)
//!   MEETIFY_PROBE_WINDOW=S-E     (required; the promoted window, seconds)
//!   MEETIFY_PROBE_SEED=S-E       (required; the seed span inside it)
//!   MEETIFY_GATE_WHISPER_MODEL   (default large-v3-turbo-q5_0)
//!   MEETIFY_GATE_LANG            (default en)
//!   MEETIFY_RENDER_PRINT         (verbatim to terminal)
//!
//! The baseline rows come from the meeting's own persisted render (the
//! rows the promotion would replace) — read-only DB access, the same
//! pattern the ear gate uses.

use app_lib::audio::speaker::net_worse_guard::{
    BaselineRow, SeedWalls, build_conservation_groups, lcs_length, normalize_tokens,
};
use app_lib::audio::speaker::ports::VoiceSeparationPort;
use app_lib::whisper_engine::whisper_engine::WhisperEngine;

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";
const SR: u32 = 16_000;

fn samples_cache() -> std::path::PathBuf {
    let meeting = std::env::var("MEETIFY_MEETING_DIR").expect("MEETIFY_MEETING_DIR (recording dir name)");
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let music = std::path::Path::new(&home).join("Music");
    for entry in std::fs::read_dir(&music).expect("read Music") {
        let p = entry.expect("dir entry").path();
        if p.is_dir() && p.join(&meeting).join("samples_16k.f32").exists() {
            return p.join(&meeting).join("samples_16k.f32");
        }
    }
    if let Ok(dir) = std::env::var("MEETIFY_RECORDINGS_DIR") {
        let p = std::path::Path::new(&dir).join(&meeting).join("samples_16k.f32");
        if p.exists() {
            return p;
        }
    }
    panic!("no samples_16k.f32 cache found for {meeting}");
}

fn parse_span(spec: &str) -> (f64, f64) {
    let (a, b) = spec
        .split_once('-')
        .unwrap_or_else(|| panic!("span {spec:?} must be START-END"));
    let (a, b) = (a.trim().parse::<f64>().expect("start"), b.trim().parse::<f64>().expect("end"));
    assert!(b > a, "span {spec:?} end <= start");
    (a, b)
}

async fn read_baseline_rows(window: (f64, f64)) -> Vec<(String, f64, f64, String)> {
    let home = std::env::var("USERPROFILE").expect("USERPROFILE");
    let db = std::path::Path::new(&home).join("AppData/Roaming/com.meetily.ai/meeting_minutes.sqlite");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .read_only(true)
                .filename(&db),
        )
        .await
        .expect("open meeting DB read-only");
    let rows = sqlx::query_as::<_, (Option<String>, f64, f64, String)>(
        "SELECT speaker_label, audio_start_time, audio_end_time, transcript \
         FROM transcripts WHERE meeting_id = ?1 AND audio_end_time > ?2 AND audio_start_time < ?3 \
         ORDER BY audio_start_time, audio_end_time",
    )
    .bind(MEETING_ID)
    .bind(window.0)
    .bind(window.1)
    .fetch_all(&pool)
    .await
    .expect("read persisted rows");
    pool.close().await;
    rows.into_iter()
        .map(|(b, s, e, t)| (b.unwrap_or_else(|| "Unknown".to_string()), s, e, t))
        .collect()
}

#[tokio::test]
#[ignore = "live GPU probe: MEETIFY_LIVE_DIAG=1 cargo test --test guard_validation_probe -- --ignored --nocapture"]
async fn guard_validation_probe() {
    if std::env::var("MEETIFY_LIVE_DIAG").is_err() {
        return;
    }
    let window = parse_span(
        &std::env::var("MEETIFY_PROBE_WINDOW").expect("MEETIFY_PROBE_WINDOW must be START-END"),
    );
    let seed = parse_span(
        &std::env::var("MEETIFY_PROBE_SEED").expect("MEETIFY_PROBE_SEED must be START-END"),
    );
    assert!(
        seed.0 >= window.0 && seed.1 <= window.1,
        "seed span must sit inside the window"
    );
    let model =
        std::env::var("MEETIFY_GATE_WHISPER_MODEL").unwrap_or_else(|_| "large-v3-turbo-q5_0".to_string());
    let lang = std::env::var("MEETIFY_GATE_LANG").unwrap_or_else(|_| "en".to_string());
    let print_verbatim = std::env::var("MEETIFY_RENDER_PRINT").is_ok();

    let models_dir = dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("models");
    let mut engine = WhisperEngine::new_with_models_dir(Some(models_dir.clone())).expect("engine ctor");
    engine.discover_models().await.expect("discover models");
    engine.load_model(&model).await.expect("load model");
    let separator =
        app_lib::audio::speaker::separation::ConvTasNetSeparator::from_models_dir().expect("separator");

    let cache = samples_cache();
    let raw = std::fs::read(&cache).expect("read samples cache");
    let all: Vec<f32> = raw
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    // Separate the promoted window into the two repair streams.
    let sep = tokio::task::spawn_blocking(move || separator.separate(&all, window))
        .await
        .expect("separate join")
        .expect("separation");

    // Decode both streams (blocking decode only — spawn_blocking-ONLY).
    let decoded = tokio::task::spawn_blocking(move || {
        sep.iter()
            .enumerate()
            .map(|(i, st)| {
                let text = engine
                    .transcribe_span_blocking(st.samples.clone(), &lang)
                    .unwrap_or_default();
                (i, st.pre_rms_ratio, text)
            })
            .collect::<Vec<_>>()
    })
    .await
    .expect("decode join");

    // The replaced mixture rows: the meeting's own persisted render.
    let db_rows = read_baseline_rows(window).await;
    let baseline_rows: Vec<BaselineRow> = db_rows
        .iter()
        .map(|(badge, s, e, text)| BaselineRow {
            badge: badge.clone(),
            start_s: *s,
            end_s: *e,
            text: text.clone(),
        })
        .collect();
    let groups = build_conservation_groups(&baseline_rows, &SeedWalls { start_s: seed.0, end_s: seed.1 });

    // Token-only report.
    eprintln!("PROBE window=[{:.2},{:.2}] seed=[{:.2},{:.2}]", window.0, window.1, seed.0, seed.1);
    eprintln!("BASELINE rows={} groups={}", db_rows.len(), groups.len());
    for (badge, s, e, text) in &db_rows {
        let toks = normalize_tokens(text);
        use sha2::{Digest, Sha256};
        let sha = format!("{:x}", Sha256::digest(text.as_bytes()));
        let shown = if print_verbatim {
            text.clone()
        } else {
            format!("<{} tokens, sha {sha}>", toks.len())
        };
        eprintln!("ROW badge={badge} walls=[{s:.2},{e:.2}] {shown}");
    }
    for g in &groups {
        eprintln!("GROUP badge={} conserved_tokens={}", g.badge, g.baseline_tokens.len());
    }
    for (i, ratio, text) in &decoded {
        let toks = normalize_tokens(text);
        use sha2::{Digest, Sha256};
        let sha = format!("{:x}", Sha256::digest(text.as_bytes()));
        let shown = if print_verbatim {
            text.clone()
        } else {
            format!("<{} tokens, sha {sha}>", toks.len())
        };
        eprintln!("STREAM i={i} rms_ratio={ratio:.3} tokens={} {shown}", toks.len());
    }
    eprintln!("MATRIX (group badge x stream)");
    for g in &groups {
        for (i, _, text) in &decoded {
            let st_tokens = normalize_tokens(text);
            let lcs = lcs_length(&g.baseline_tokens, &st_tokens);
            let recall = if g.baseline_tokens.is_empty() {
                0.0
            } else {
                lcs as f64 / g.baseline_tokens.len() as f64
            };
            let precision = if st_tokens.is_empty() {
                0.0
            } else {
                lcs as f64 / st_tokens.len() as f64
            };
            eprintln!(
                "MATRIX group={} stream{i} lcs={lcs} recall={recall:.3} precision={precision:.3}",
                g.badge
            );
        }
        // Best assignment for this group (the vote-resolved badge picks one
        // stream; the best row shows the statistic's upper bound).
        let best = decoded
            .iter()
            .map(|(i, _, text)| {
                let st_tokens = normalize_tokens(text);
                (
                    i,
                    lcs_length(&g.baseline_tokens, &st_tokens) as f64
                        / g.baseline_tokens.len().max(1) as f64,
                )
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("streams");
        eprintln!(
            "BEST group={} stream{} recall={:.3}",
            g.badge, best.0, best.1
        );
    }
    if groups.is_empty() {
        eprintln!("EMPTY conservation set — the guard would refuse this promotion");
    }
}
