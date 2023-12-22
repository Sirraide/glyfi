use const_format::formatcp;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use glyfi_sql_defs::DB_PATH;

#[tokio::main]
async fn main() {
    // Create the database if it doesn’t exist yet.
    println!("Initialising db...");
    if let Err(e) = Sqlite::create_database(DB_PATH).await {
        panic!("Failed to open db connexion: {}", e);
    }

    // Create DB connexion.
    let pool = SqlitePool::connect(DB_PATH).await.unwrap();

    // Create submissions table.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS submissions (
            message INTEGER PRIMARY KEY, -- Message ID of the submission.
            week INTEGER NOT NULL, -- This is just an integer.
            challenge INTEGER NOT NULL, -- See Challenge enum.
            author INTEGER NOT NULL, -- Discord user ID of the author.
            link TEXT NOT NULL, -- Link to the submission.
            time INTEGER NOT NULL DEFAULT (unixepoch()), -- Time of submission.
            votes INTEGER NOT NULL DEFAULT 0 -- Number of votes.
        ) STRICT;
    "#).execute(&pool).await.unwrap();

    // The current week. This is a table with a single entry.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS current_week (
            week INTEGER NOT NULL
        ) STRICT;
    "#).execute(&pool).await.unwrap();

    // Prevent inserting additional weeks.
    sqlx::query(formatcp!(r#"
        CREATE TRIGGER IF NOT EXISTS current_week_insertion
        BEFORE INSERT ON current_week
        WHEN (SELECT COUNT(*) FROM current_week) > 0
        BEGIN
            SELECT RAISE(ABORT, "current_week table must not contain more than one entry!");
        END;
    "#)).execute(&pool).await.unwrap();

    // The user is expected to set this manually, but ensure it exists. This
    // is allowed to fail due to the trigger above.
    let _ = sqlx::query("INSERT OR IGNORE INTO current_week (week) VALUES (0)").execute(&pool).await;

    // Table that stores what weeks are/were regular, special, or extended.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS weeks (
            week INTEGER PRIMARY KEY, -- Week number.
            kind INTEGER NOT NULL -- See Week enum.
        ) STRICT;
    "#).execute(&pool).await.unwrap();

    // Merge everything into one db file.
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&pool).await.unwrap();
    pool.close().await;
}