//! The real migration chain applies cleanly to a fresh database and the
//! engine's continuation-fact column exists on `transcripts`
//! (change `hybrid-diarization-engine`, task 4.4). Plain `cargo test` —
//! no audio, models, or env gate.

use sqlx::Row;

#[tokio::test]
async fn migration_chain_applies_and_transcripts_has_continues_previous() {
    // Single connection: `:memory:` databases are per-connection in SQLite.
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(":memory:")
        .await
        .expect("in-memory pool");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("all migrations apply cleanly");

    let row = sqlx::query(
        "SELECT type, \"notnull\" FROM pragma_table_info('transcripts') WHERE name = 'continues_previous'",
    )
    .fetch_one(&pool)
    .await
    .expect("continues_previous column exists on transcripts");
    assert_eq!(row.get::<String, _>("type"), "INTEGER");
    assert_eq!(row.get::<i64, _>("notnull"), 0, "column must be nullable (legacy rows = null flag)");
}
