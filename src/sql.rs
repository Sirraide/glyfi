use const_format::formatcp;
use poise::serenity_prelude::{MessageId, UserId};
use sqlx::migrate::MigrateDatabase;
use sqlx::{FromRow, Sqlite, SqlitePool};
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

/// Profile for a user.
#[derive(Clone, Debug, FromRow)]
pub struct UserProfileData {
    pub nickname: Option<String>,

    /// Number of 1st, 2nd, 3rd place finishes in the Glyphs Challenge.
    pub glyphs_first: i64,
    pub glyphs_second: i64,
    pub glyphs_third: i64,

    /// Number of 1st, 2nd, 3rd place finishes in the Ambigram Challenge.
    pub ambigrams_first: i64,
    pub ambigrams_second: i64,
    pub ambigrams_third: i64,

    /// Highest ranking in either challenge.
    pub highest_ranking_glyphs: i64,
    pub highest_ranking_ambigrams: i64,

    /// Number of submissions.
    pub glyphs_submissions: i64,
    pub ambigrams_submissions: i64,
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

    // Cached user profile data (excludes current week, obviously).
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY, -- Discord user ID.
            nickname TEXT, -- Nickname.

            -- Number of 1st, 2nd, 3rd place finishes in the Glyphs Challenge.
            glyphs_first INTEGER NOT NULL DEFAULT 0,
            glyphs_second INTEGER NOT NULL DEFAULT 0,
            glyphs_third INTEGER NOT NULL DEFAULT 0,

            -- Number of 1st, 2nd, 3rd place finishes in the Ambigram Challenge.
            ambigrams_first INTEGER NOT NULL DEFAULT 0,
            ambigrams_second INTEGER NOT NULL DEFAULT 0,
            ambigrams_third INTEGER NOT NULL DEFAULT 0,

            -- Highest ranking in either challenge.
            highest_ranking_glyphs INTEGER NOT NULL DEFAULT 0,
            highest_ranking_ambigrams INTEGER NOT NULL DEFAULT 0
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

/// Get profile data for a user.
pub async fn get_user_profile(user: UserId) -> Result<UserProfileData, Error> {
    #[derive(Default, FromRow)]
    pub struct UserProfileDataFirst {
        pub nickname: Option<String>,
        pub glyphs_first: i64,
        pub glyphs_second: i64,
        pub glyphs_third: i64,
        pub ambigrams_first: i64,
        pub ambigrams_second: i64,
        pub ambigrams_third: i64,
        pub highest_ranking_glyphs: i64,
        pub highest_ranking_ambigrams: i64,
    }

    #[derive(Default, FromRow)]
    pub struct UserProfileDataSecond {
        pub glyphs_submissions: i64,
        pub ambigrams_submissions: i64,
    }

    let first: UserProfileDataFirst = sqlx::query_as(r#"
        SELECT
            nickname,
            glyphs_first, glyphs_second, glyphs_third,
            ambigrams_first, ambigrams_second, ambigrams_third,
            highest_ranking_glyphs, highest_ranking_ambigrams
        FROM users
        WHERE id = ?;
    "#)
        .bind(user.get() as i64)
        .fetch_optional(pool())
        .await
        .map_err(|e| format!("Failed to get user profile data: {}", e))?
        .unwrap_or_default();

    let second: UserProfileDataSecond = sqlx::query_as(formatcp!(r#"
        SELECT
            SUM(IIF(challenge = {}, 1, 0)) as glyphs_submissions,
            SUM(IIF(challenge = {}, 1, 0)) as ambigrams_submissions
        FROM submissions
        WHERE author = ?
        GROUP BY author;
    "#, Challenge::Glyph as i64, Challenge::Ambigram as i64))
        .bind(user.get() as i64)
        .fetch_optional(pool())
        .await
        .map_err(|e| format!("Failed to get user profile data: {}", e))?
        .unwrap_or_default();

    Ok(UserProfileData {
        nickname: first.nickname,

        glyphs_first: first.glyphs_first,
        glyphs_second: first.glyphs_second,
        glyphs_third: first.glyphs_third,

        ambigrams_first: first.ambigrams_first,
        ambigrams_second: first.ambigrams_second,
        ambigrams_third: first.ambigrams_third,

        highest_ranking_glyphs: first.highest_ranking_glyphs,
        highest_ranking_ambigrams: first.highest_ranking_ambigrams,

        glyphs_submissions: second.glyphs_submissions,
        ambigrams_submissions: second.ambigrams_submissions,
    })
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