use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};
use glyfi_sql_defs::DB_PATH;
use crate::info_sync;

static mut __GLYFI_DB_POOL: Option<SqlitePool> = None;

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
}

/// Get the global sqlite connexion pool.
fn pool() -> &'static SqlitePool {
    unsafe { __GLYFI_DB_POOL.as_ref().unwrap() }
}

