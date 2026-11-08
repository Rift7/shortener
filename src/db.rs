use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS links (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    slug       TEXT    NOT NULL UNIQUE,
    url        TEXT    NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
CREATE INDEX IF NOT EXISTS idx_links_url ON links (url);
"#;

/// Open (creating if needed) the SQLite database and apply the schema.
pub async fn connect(path: &str) -> Result<SqlitePool> {
    if path != ":memory:" {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .with_context(|| format!("creating database directory {}", parent.display()))?;
            }
        }
    }

    let options = SqliteConnectOptions::from_str(path)
        .with_context(|| format!("invalid SQLite path: {path}"))?
        .create_if_missing(path != ":memory:")
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .context("opening SQLite database")?;

    sqlx::raw_sql(SCHEMA)
        .execute(&pool)
        .await
        .context("applying schema")?;

    Ok(pool)
}

/// SQLite reports unique-constraint violations as extended code 2067.
pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    match err.as_database_error() {
        Some(db_err) => {
            db_err.code().as_deref() == Some("2067")
                || db_err.message().to_ascii_lowercase().contains("unique")
        }
        None => false,
    }
}
