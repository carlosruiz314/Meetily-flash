//! Task 1.4 (change `align-from-immutable-source`): migration + dual-write
//! storage guarantees.
//!
//! - The eager backfill copies every `transcripts` row verbatim with
//!   `source_origin = 'backfilled'` (design D2).
//! - A mid-backfill failure rolls the DB back to the pre-migration schema
//!   (sqlx wraps each migration in one transaction).
//! - Meeting deletion through the production repository path removes the
//!   meeting's source rows explicitly (design D1).
//! - The dual-write helper leaves zero rows in either table when the
//!   transaction rolls back (design D3).

use app_lib::api::TranscriptSegment;
use app_lib::database::repositories::meeting::MeetingsRepository;
use app_lib::database::repositories::transcript::TranscriptsRepository;

const MIGRATION_SQL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/migrations/20260909000000_add_transcript_sources.sql"
));

/// The migration file's two statements (CREATE TABLE, backfill INSERT),
/// extracted by keyword up to the terminating ';' — the file's comments
/// contain semicolons, so a naive split would cut mid-comment.
fn migration_statements() -> Vec<String> {
    let mut stmts = Vec::new();
    for keyword in ["CREATE TABLE", "INSERT INTO"] {
        let start = MIGRATION_SQL.find(keyword).expect("migration keyword");
        let rest = &MIGRATION_SQL[start..];
        let end = rest.find(';').expect("statement terminator") + 1;
        stmts.push(rest[..end].to_string());
    }
    assert_eq!(stmts.len(), 2, "migration file shape changed — update this test");
    stmts
}

/// Pre-migration schema: meetings + the `transcripts` table with the FULL
/// column set as it stood just before this migration (initial schema + the
/// speaker/audio/token ALTER migrations).
async fn pre_migration_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    for ddl in [
        "CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        "CREATE TABLE transcripts (
            id TEXT PRIMARY KEY,
            meeting_id TEXT NOT NULL,
            transcript TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            summary TEXT, action_items TEXT, key_points TEXT,
            speaker TEXT,
            audio_start_time REAL, audio_end_time REAL, duration REAL,
            speaker_label TEXT, token_timestamps TEXT, speaker_source TEXT,
            previous_label TEXT, continues_previous INTEGER,
            FOREIGN KEY (meeting_id) REFERENCES meetings(id) ON DELETE CASCADE
        )",
    ] {
        sqlx::query(ddl).execute(&pool).await.unwrap();
    }
    pool
}

/// Seed one meeting with two transcription rows: one full row (token JSON,
/// summary fields) and one NULL-token row (the replaced-row signature).
async fn seed_pre_migration_rows(pool: &sqlx::SqlitePool) {
    sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('m1', 't', 'now', 'now')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, meeting_id, transcript, timestamp, summary, action_items, key_points) \
         VALUES ('t-1', 'm1', 'Where is Ricardo?', '00:32', 's', 'a', 'k')",
    )
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO transcripts (id, meeting_id, transcript, timestamp) \
         VALUES ('t-2', 'm1', 'fused degraded row.', '00:40')",
    )
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn backfill_copies_rows_verbatim_with_backfilled_origin() {
    let pool = pre_migration_pool().await;
    seed_pre_migration_rows(&pool).await;

    for stmt in migration_statements() {
        sqlx::raw_sql(&stmt).execute(&pool).await.unwrap();
    }

    let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, meeting_id, transcript, summary, source_origin \
         FROM transcript_sources ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        rows,
        vec![
            (
                "t-1".into(),
                "m1".into(),
                "Where is Ricardo?".into(),
                Some("s".into()),
                "backfilled".into()
            ),
            (
                "t-2".into(),
                "m1".into(),
                "fused degraded row.".into(),
                None,
                "backfilled".into()
            ),
        ],
        "backfill copies rows verbatim, ids preserved, nothing stamped 'stt'"
    );

    // The per-meeting NULL-token counts the app logs at upgrade time
    // (manager.rs) — here both rows are NULL-token (pre-token era schema).
    let counts: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT meeting_id, COUNT(*), SUM(CASE WHEN token_timestamps IS NULL THEN 1 ELSE 0 END) \
         FROM transcript_sources WHERE source_origin = 'backfilled' GROUP BY meeting_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(counts, vec![("m1".into(), 2, 2)]);
}

#[tokio::test]
async fn migration_failure_mid_backfill_rolls_back_to_pre_migration_schema() {
    let pool = pre_migration_pool().await;
    seed_pre_migration_rows(&pool).await;

    // sqlx runs each migration in ONE transaction (design D2): DDL and the
    // backfill together. Model that — both statements run inside the tx.
    let stmts = migration_statements();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(&stmts[0]).execute(&mut *tx).await.unwrap();
    // Fault injection: the backfill INSERT aborts on one specific row.
    sqlx::query(
        "CREATE TRIGGER fail_backfill BEFORE INSERT ON transcript_sources \
         WHEN NEW.id = 't-2' BEGIN SELECT RAISE(ABORT, 'injected backfill failure'); END",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    let res = sqlx::raw_sql(&stmts[1]).execute(&mut *tx).await;
    assert!(res.is_err(), "injected RAISE(ABORT) must fail the backfill");
    drop(tx); // explicit rollback

    // The transaction rolled back: the pre-migration schema state is intact —
    // the new table is gone and the source rows are untouched.
    let (table_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'transcript_sources'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(table_count, 0, "rolled-back DDL leaves no transcript_sources table");
    let (rows,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transcripts").fetch_one(&pool).await.unwrap();
    assert_eq!(rows, 2, "transcripts rows untouched");
}

#[tokio::test]
async fn delete_meeting_removes_source_rows_through_production_path() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    for ddl in [
        "CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, folder_path TEXT)",
        "CREATE TABLE transcript_chunks (id TEXT PRIMARY KEY, meeting_id TEXT)",
        "CREATE TABLE summary_processes (id TEXT PRIMARY KEY, meeting_id TEXT)",
        "CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL)",
        "CREATE TABLE transcript_sources (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL)",
    ] {
        sqlx::query(ddl).execute(&pool).await.unwrap();
    }
    sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('m1', 'a', 'now', 'now')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('m2', 'b', 'now', 'now')")
        .execute(&pool)
        .await
        .unwrap();
    for table in ["transcripts", "transcript_sources"] {
        sqlx::query(&format!("INSERT INTO {table} (id, meeting_id) VALUES ('t-1', 'm1')"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(&format!("INSERT INTO {table} (id, meeting_id) VALUES ('t-2', 'm1')"))
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(&format!("INSERT INTO {table} (id, meeting_id) VALUES ('t-3', 'm2')"))
            .execute(&pool)
            .await
            .unwrap();
    }

    let deleted = MeetingsRepository::delete_meeting(&pool, "m1").await.unwrap();
    assert!(deleted);

    // Explicit delete beside transcripts — the FK cascade is never relied on.
    let (sources_left,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transcript_sources WHERE meeting_id = 'm1'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sources_left, 0, "meeting deletion removes the meeting's source rows");
    let (other_meeting,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM transcript_sources WHERE meeting_id = 'm2'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(other_meeting, 1, "other meetings' source rows survive");
}

#[tokio::test]
async fn dual_write_helper_rolls_back_both_tables_on_failure() {
    let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
    // transcript_sources deliberately LACKS token_timestamps: the rendering
    // insert succeeds, the source insert fails, and the whole transaction
    // (meeting row included) must roll back.
    for ddl in [
        "CREATE TABLE meetings (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, folder_path TEXT)",
        "CREATE TABLE transcripts (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, audio_start_time REAL, audio_end_time REAL, duration REAL, token_timestamps TEXT)",
        "CREATE TABLE transcript_sources (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, transcript TEXT NOT NULL, timestamp TEXT NOT NULL, audio_start_time REAL, audio_end_time REAL, duration REAL, source_origin TEXT NOT NULL DEFAULT 'stt')",
    ] {
        sqlx::query(ddl).execute(&pool).await.unwrap();
    }

    let segment = TranscriptSegment {
        id: "seg-1".into(),
        text: "dual-written text".into(),
        timestamp: "00:01".into(),
        audio_start_time: Some(0.0),
        audio_end_time: Some(1.0),
        duration: Some(1.0),
        token_timestamps: Some("[]".into()),
    };
    let err = TranscriptsRepository::save_transcript(
        &pool,
        "meeting-11111111-2222-4333-8444-555555555555",
        "title",
        &[segment],
        None,
    )
    .await;
    assert!(err.is_err(), "source insert failure must surface");

    for table in ["meetings", "transcripts", "transcript_sources"] {
        let (n,): (i64,) =
            sqlx::query_as(&format!("SELECT COUNT(*) FROM {table}")).fetch_one(&pool).await.unwrap();
        assert_eq!(n, 0, "{table} must be empty after the rolled-back dual-write");
    }
}
