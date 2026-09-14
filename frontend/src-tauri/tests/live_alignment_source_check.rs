//! Task 3.3 live check (change `align-from-immutable-source`), driven headless
//! against the REAL app DB — no UI click. Two parts:
//!
//! 1. `migrate_production_and_verify_source_parity`: applies the canonical
//!    sqlx migrations to the production DB (what the app does on launch),
//!    prints the D2 per-meeting backfill log, then proves the meeting's new
//!    source equals its degraded rendering ROW-FOR-ROW against no-split's
//!    pinned snapshot fixture.
//! 2. `live_double_run_source_byte_stability`: two consecutive full Speakers
//!    runs (run_speakers_reset_standalone, the Speakers-button path) with the
//!    source hash asserted byte-identical across both runs — the headline
//!    invariant: no run's output feeds any later run's input — and the
//!    rendering asserted structurally stable (row count + text multiset;
//!    badge jitter printed for the record, not gated).
//!
//! Run (each takes minutes to ~1 h; Speakers runs recompute the engine):
//!   cargo test --release --test live_alignment_source_check -- --ignored --nocapture

use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};

const MEETING_ID: &str = "meeting-cde5c264-1c4a-49d9-97c5-6a7e69bb9323";

fn app_db_path() -> std::path::PathBuf {
    dirs::data_dir()
        .expect("no user data dir")
        .join("com.meetily.ai")
        .join("meeting_minutes.sqlite")
}

async fn app_pool() -> sqlx::SqlitePool {
    let db = app_db_path();
    assert!(db.exists(), "app DB not found at {}", db.display());
    sqlx::sqlite::SqlitePool::connect(db.to_str().unwrap()).await.unwrap()
}

/// One `transcript_sources` row in fixture-compatible shape.
#[derive(Debug, Clone, sqlx::FromRow)]
struct SourceRow {
    id: String,
    transcript: String,
    audio_start_time: f64,
    audio_end_time: f64,
    token_timestamps: Option<String>,
}

async fn read_source_rows(pool: &sqlx::SqlitePool) -> Vec<SourceRow> {
    sqlx::query_as::<_, SourceRow>(
        "SELECT id, transcript, audio_start_time, audio_end_time, token_timestamps \
         FROM transcript_sources WHERE meeting_id = ? ORDER BY id",
    )
    .bind(MEETING_ID)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Canonical hash over the source rows — the same scheme as the no-split
/// gate's `rows_sha256` (rows ordered by (start_ms, end_ms, id); per row
/// `id \x1f text \x1f start_ms \x1f end_ms \x1f token_json_or_empty`, joined
/// by \x1e) so the result is directly comparable with the pinned fixture.
fn source_hash(rows: &[SourceRow]) -> String {
    let mut sorted: Vec<&SourceRow> = rows.iter().collect();
    sorted.sort_by(|a, b| {
        let ka = ((a.audio_start_time * 1000.0) as i64, (a.audio_end_time * 1000.0) as i64, a.id.as_str());
        let kb = ((b.audio_start_time * 1000.0) as i64, (b.audio_end_time * 1000.0) as i64, b.id.as_str());
        ka.cmp(&kb)
    });
    let canon: Vec<String> = sorted
        .iter()
        .map(|r| {
            format!(
                "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
                r.id,
                r.transcript,
                (r.audio_start_time * 1000.0) as i64,
                (r.audio_end_time * 1000.0) as i64,
                r.token_timestamps.as_deref().unwrap_or("")
            )
        })
        .collect();
    let mut h = Sha256::new();
    h.update(canon.join("\u{1e}").as_bytes());
    format!("{:x}", h.finalize())
}

async fn threshold_fp_from_db(pool: &sqlx::SqlitePool) -> u32 {
    let threshold: f64 = sqlx::query("SELECT speakerMergeThreshold FROM settings LIMIT 1")
        .fetch_optional(pool)
        .await
        .unwrap()
        .map(|r| sqlx::Row::get::<f64, _>(&r, "speakerMergeThreshold"))
        .unwrap_or(0.40);
    (threshold as f32 * 65536.0) as u32
}

/// Part 1 — migrate the production DB (canonical migrator), log the D2
/// backfill report, verify source == degraded rendering against no-split's
/// pinned snapshot.
#[tokio::test]
#[ignore = "writes migrations to the REAL app DB; run explicitly with --ignored"]
async fn migrate_production_and_verify_source_parity() {
    let pool = app_pool().await;

    // Apply the canonical migration set — identical to the app-launch path.
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // D2 backfill report (the same query manager.rs logs at upgrade time).
    let counts: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT meeting_id, COUNT(*), \
         SUM(CASE WHEN token_timestamps IS NULL THEN 1 ELSE 0 END) \
         FROM transcript_sources WHERE source_origin = 'backfilled' \
         GROUP BY meeting_id ORDER BY meeting_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    println!("BACKFILL REPORT: {} meeting(s) seeded", counts.len());
    for (meeting_id, total, null_tokens) in &counts {
        println!("  {meeting_id}: {total} row(s), {null_tokens} NULL-token (replaced-row signature)");
    }

    // Provenance: every seeded row is 'backfilled'.
    let (bad_origin,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transcript_sources WHERE source_origin NOT IN ('stt', 'backfilled')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(bad_origin, 0);

    // Parity: the meeting's source equals its (degraded) rendering.
    let (src_n,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM transcript_sources WHERE meeting_id = ?",
    )
    .bind(MEETING_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (rnd_n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transcripts WHERE meeting_id = ?")
            .bind(MEETING_ID)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(src_n, rnd_n, "post-backfill source rows == rendering rows");
    let mismatched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcript_sources s JOIN transcripts t ON s.id = t.id \
         WHERE s.meeting_id = ? AND (s.transcript != t.transcript \
         OR s.audio_start_time != t.audio_start_time OR s.audio_end_time != t.audio_end_time)",
    )
    .bind(MEETING_ID)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(mismatched, 0, "source rows equal their rendering twins");

    // Row-for-row match against no-split's pinned snapshot (the SHA-pinned
    // freeze this archive left behind).
    let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cde5c264_transcripts.json");
    #[derive(serde::Deserialize)]
    struct FixRow {
        id: String,
        text: String,
        start_ms: i64,
        end_ms: i64,
        #[serde(default)]
        token_timestamps: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct Fixture {
        meeting: String,
        row_sha256: String,
        rows: Vec<FixRow>,
    }
    let fixture: Fixture = serde_json::from_str(
        &std::fs::read_to_string(&fixture_path).expect("read pinned snapshot"),
    )
    .expect("parse pinned snapshot");
    assert_eq!(fixture.meeting, MEETING_ID);

    let source = read_source_rows(&pool).await;
    assert_eq!(source.len(), fixture.rows.len(), "source row count == pinned snapshot");
    let hash = source_hash(&source);
    println!(
        "SOURCE HASH {hash} vs PINNED {}",
        fixture.row_sha256
    );
    // The fixture pins the pipeline's ACTUAL input; equality here means the
    // gate's cross-check is already consistent post-backfill.
    assert_eq!(hash, fixture.row_sha256, "backfilled source hash == pinned snapshot sha");

    println!("PARITY OK: source == degraded rendering == pinned snapshot ({src_n} rows)");
    pool.close().await;
}

/// Part 2 — the double-run stability check. Source byte-identical across two
/// full Speakers runs; rendering structurally stable (row count + text
/// multiset); badge distribution printed for the jitter record.
#[tokio::test]
#[ignore = "drives the REAL app DB + REAL recording (~1 h); run explicitly with --ignored"]
async fn live_double_run_source_byte_stability() {
    std::env::set_var("RUST_LOG", "info");
    let _ = env_logger::try_init();
    let pool = app_pool().await;
    let threshold_fp = threshold_fp_from_db(&pool).await;

    async fn rendering_state(pool: &sqlx::SqlitePool) -> (i64, Vec<String>, Vec<(String, i64)>) {
        #[derive(sqlx::FromRow)]
        struct RenderRowState {
            transcript: String,
            speaker_label: Option<String>,
        }
        let rows = sqlx::query_as::<_, RenderRowState>(
            "SELECT transcript, speaker_label FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time, id",
        )
        .bind(MEETING_ID)
        .fetch_all(pool)
        .await
        .unwrap();
        let mut texts: Vec<String> = rows.iter().map(|r| r.transcript.clone()).collect();
        texts.sort();
        let mut badges: std::collections::BTreeMap<String, i64> = Default::default();
        for r in &rows {
            *badges.entry(r.speaker_label.clone().unwrap_or_default()).or_default() += 1;
        }
        (rows.len() as i64, texts, badges.into_iter().collect())
    }

    async fn run_once(pool: &sqlx::SqlitePool, threshold_fp: u32, label: &str) {
        let registry: Arc<
            Mutex<Option<app_lib::audio::speaker::sherpa_adapter::CosineRegistryAdapter>>,
        > = Arc::new(Mutex::new(None));
        let t0 = std::time::Instant::now();
        let result = app_lib::audio::speaker::commands::run_speakers_reset_standalone(
            pool,
            MEETING_ID,
            threshold_fp,
            registry,
        )
        .await
        .unwrap_or_else(|e| panic!("{label} failed: {e}"));
        println!(
            "{label}: {:.1}s — {} speakers, {} segments labeled, unmatched={:?}",
            t0.elapsed().as_secs_f64(),
            result.speaker_count,
            result.segments_labeled,
            result.unmatched_manual_names
        );
    }

    // Run 1.
    let source_before = read_source_rows(&pool).await;
    let hash_before = source_hash(&source_before);
    println!("source before run 1: {} rows, sha {hash_before}", source_before.len());
    run_once(&pool, threshold_fp, "RUN 1").await;
    let source_after_1 = read_source_rows(&pool).await;
    let hash_after_1 = source_hash(&source_after_1);
    assert_eq!(
        hash_after_1, hash_before,
        "THE headline invariant: the Speakers run did not touch the immutable source"
    );
    let (count_1, texts_1, badges_1) = rendering_state(&pool).await;
    println!("rendering after run 1: {count_1} rows, badges {badges_1:?}");

    // Run 2 — same source, same path.
    run_once(&pool, threshold_fp, "RUN 2").await;
    let source_after_2 = read_source_rows(&pool).await;
    assert_eq!(
        source_hash(&source_after_2),
        hash_before,
        "source still byte-identical after the second run"
    );
    let (count_2, texts_2, badges_2) = rendering_state(&pool).await;

    println!(
        "STABILITY: run1 {count_1} rows vs run2 {count_2} rows; badges {badges_1:?} vs {badges_2:?}"
    );
    assert_eq!(count_1, count_2, "structural stability: row count");
    assert_eq!(texts_1, texts_2, "structural stability: text multiset");
    if badges_1 != badges_2 {
        println!("BADGE JITTER (investigate, not gated): {badges_1:?} vs {badges_2:?}");
    }

    // The proposal's headline sentence is intact and splittable in the source.
    let ricardo: Vec<String> = sqlx::query_scalar(
        "SELECT transcript FROM transcript_sources WHERE meeting_id = ? AND transcript LIKE 'Where is Ricardo?%'",
    )
    .bind(MEETING_ID)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(
        !ricardo.is_empty(),
        "'Where is Ricardo?' survives with its '?' in the immutable source"
    );
    println!("RICARDO SOURCE ROW: {:?}", ricardo[0]);

    pool.close().await;
}
