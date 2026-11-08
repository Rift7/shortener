use anyhow::Result;
use shortener::{build_router, build_state, Config};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("shortener=info,tower_http=info,warn")),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cfg = Config::from_env()?;
    let bind = cfg.bind;
    let base_url = cfg.public_base_url.clone();
    let db_path = cfg.db_path.clone();
    let key_count = cfg.api_keys.len();

    let state = build_state(cfg).await?;
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(
        %bind, %base_url, %db_path, keys = key_count,
        "shortener ready"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("shortener stopped");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("shutdown signal received, draining connections");
}
