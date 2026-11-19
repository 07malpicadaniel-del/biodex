//! Executable entry point for `biodex-backend`.

use std::sync::Arc;
use std::time::Duration;

use biodex_backend::almanac::sync_almanac_catalog;
use biodex_backend::config::BackendConfig;
use biodex_backend::routes::{AppState, router};
use biodex_backend::vision::INaturalistVisionClient;
use biodex_backend::{load_mods_from_dir, run_migrations};
use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("biodex_backend=info,tower_http=info")),
        )
        .init();

    let config = BackendConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    run_migrations(&pool).await?;

    let mods = Arc::new(load_mods_from_dir(&config.mods_dir)?);
    tracing::info!(packs = mods.packs().len(), "loaded mod packs");

    let synced = sync_almanac_catalog(&pool, &mods).await?;
    tracing::info!(species = synced, "synchronized core almanac catalog");

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("BioDex/0.1.0 (Open Source Citizen Science)")
        .build()?;

    let vision = Arc::new(INaturalistVisionClient::new(
        http.clone(),
        config.inaturalist_api_token,
        config.allow_dev_taxon_hint,
    ));

    let state = AppState {
        db: pool,
        http,
        mods,
        vision,
        max_moire_score: config.max_moire_score,
    };

    let listener = TcpListener::bind(&config.bind_addr).await?;
    tracing::info!(addr = %config.bind_addr, "BioDex backend listening");

    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
    tracing::info!("shutdown signal received");
}
