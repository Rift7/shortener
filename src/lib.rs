use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use sqlx::SqlitePool;

pub mod config;
pub mod db;
pub mod error;
pub mod routes;
pub mod slug;
pub mod state;
pub mod url;

pub use config::Config;
pub use state::AppState;

/// Build application state (database pool + config) for serving or testing.
pub async fn build_state(cfg: Config) -> Result<Arc<AppState>> {
    let pool = db::connect(&cfg.db_path).await?;
    Ok(Arc::new(AppState {
        cfg: Arc::new(cfg),
        db: pool,
    }))
}

/// Convenience for tests that already own a pool.
pub fn state_with_pool(cfg: Config, db: SqlitePool) -> Arc<AppState> {
    Arc::new(AppState {
        cfg: Arc::new(cfg),
        db,
    })
}

pub fn build_router(state: Arc<AppState>) -> Router {
    routes::router(state)
}
