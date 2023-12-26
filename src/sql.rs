use poise::serenity_prelude::{MessageId, UserId};
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use crate::{Error, info_sync, Res};

pub const DB_PATH: &str = "glyfi.db";

/// What challenge a submission belongs to.
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Challenge {
    Glyph = 0,
    Ambigram = 1,
}

/// Determines what kind of actions should be taken in a week.
///
/// Every week, we need to perform the following actions for
/// each challenge:
///
/// - Make an announcement post that describes that week’s challenge.
/// - Post a panel containing all submissions from the previous week.
/// - Post the top 3 submissions from the week before that.
///
/// Some weeks, however, are special in that we don’t want to take
/// one or more of those actions. A week can either be ‘regular’ or
/// ‘special’.
///
/// At the ‘beginning’ of the week (that is, the day the announcement
/// is made) we need to:
///
/// - Make a new announcement post for the current week, unless this
///   week is special.
///
/// - Post a panel containing all submissions from the previous week,
///   unless that week was special.
///
/// - Post the top three from the week before the last.
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Week {
    Regular = 0,
    Special = 1,
}

static mut __GLYFI_DB_POOL: Option<SqlitePool> = None;

/// Get the global sqlite connexion pool.
fn pool() -> &'static SqlitePool {
    unsafe { __GLYFI_DB_POOL.as_ref().unwrap() }
}

/*/// Merge the DB into one file.
pub async fn truncate_wal() {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(pool()).await.unwrap();
}
*/

/// Only intended to be called by [`terminate()`].
pub async unsafe fn __glyfi_fini_db() {
    if let Some(pool) = __GLYFI_DB_POOL.as_ref() { pool.close().await; }
}

/// Only intended to be called by main().
pub async unsafe fn __glyfi_init_db() {
    // Create the database if it doesn’t exist yet.
    info_sync!("Initialising sqlite db...");
    if let Err(e) = Sqlite::create_database(DB_PATH).await {
        panic!("Failed to create sqlite db: {}", e);
    }

    // Create DB connexion.
    __GLYFI_DB_POOL = Some(SqlitePool::connect(DB_PATH).await.unwrap());

    // Create submissions table.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS submissions (
            message INTEGER, -- Message ID of the submission.
            week INTEGER NOT NULL, -- This is just an integer.
            challenge INTEGER NOT NULL, -- See Challenge enum.
            author INTEGER NOT NULL, -- Discord user ID of the author.
            link TEXT NOT NULL, -- Link to the submission.
            time INTEGER NOT NULL DEFAULT (unixepoch()), -- Time of submission.
            votes INTEGER NOT NULL DEFAULT 0, -- Number of votes.
            PRIMARY KEY (message, week, challenge)
        ) STRICT;
    "#).execute(pool()).await.unwrap();

    // The current week. This is a table with a single entry.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS current_week (
            week INTEGER NOT NULL
        ) STRICT;
    "#).execute(pool()).await.unwrap();

    // Prevent inserting additional weeks.
    sqlx::query(r#"
        CREATE TRIGGER IF NOT EXISTS current_week_insertion
        BEFORE INSERT ON current_week
        WHEN (SELECT COUNT(*) FROM current_week) > 0
        BEGIN
            SELECT RAISE(ABORT, "current_week table must not contain more than one entry!");
        END;
    "#).execute(pool()).await.unwrap();

    // The user is expected to set this manually, but ensure it exists. This
    // is allowed to fail due to the trigger above.
    let _ = sqlx::query("INSERT OR IGNORE INTO current_week (week) VALUES (0)").execute(pool()).await;

    // Table that stores what weeks are/were regular or special.
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS weeks (
            week INTEGER PRIMARY KEY, -- Week number.
            kind INTEGER NOT NULL -- See Week enum.
        ) STRICT;
    "#).execute(pool()).await.unwrap();
}

/// Add a submission.
pub async fn add_submission(
    message: MessageId,
    challenge: Challenge,
    author: UserId,
    link: &str,
) -> Res {
    sqlx::query(r#"
        INSERT INTO submissions (
            message,
            week,
            challenge,
            author,
            link
        ) VALUES (?, ?, ?, ?, ?);
    "#)
    .bind(message.get() as i64)
    .bind(current_week().await?)
    .bind(challenge as i64)
    .bind(author.get() as i64)
    .bind(link)
    .execute(pool())
    .await
    .map(|_| ())
    .map_err(|e| e.into())
}

/// Get the current week.
pub async fn current_week() -> Result<i64, Error> {
    sqlx::query_scalar("SELECT week FROM current_week LIMIT 1;")
        .fetch_one(pool())
        .await
        .map_err(|e| format!("Failed to get current week: {}", e).into())
}

/// Remove a submission for the current week.
pub async fn remove_submission(message: MessageId, challenge: Challenge) -> Res {
    sqlx::query(r#"
        DELETE FROM submissions
        WHERE message = ?
        AND week = ?
        AND challenge = ?;
    "#)
    .bind(message.get() as i64)
    .bind(current_week().await?)
    .bind(challenge as i64)
    .execute(pool())
    .await
    .map(|_| ())
    .map_err(|e| e.into())
}