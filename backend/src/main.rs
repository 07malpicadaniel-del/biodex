//! Executable entry point for `biodex-backend`.

use std::sync::Arc;
use std::time::Duration;

use biodex_backend::almanac::{
    NaturaListaSyncOptions, sync_almanac_catalog, sync_almanac_media,
    sync_naturalista_clade_species,
};
use biodex_backend::config::{BackendConfig, VisionProviderKind};
use biodex_backend::routes::{AppState, router};
use biodex_backend::vision::{INaturalistVisionClient, MockVisionClient, TaxonomyClassifier};
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
        .timeout(Duration::from_secs(15))
        .user_agent("BioDex/0.1.0 (Open Source Citizen Science)")
        .build()?;

    match sync_almanac_media(&pool, &http, &config.endpoints.inaturalist_base_url).await {
        Ok(updated) => tracing::info!(updated, "synced NaturaLista imagery metadata"),
        Err(err) => tracing::warn!(error = %err, "NaturaLista imagery sync skipped/deferred"),
    }

    let naturalista_synced_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM species_catalog WHERE is_naturalista_synced = TRUE",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    if naturalista_synced_count == 0 {
        let sync_pool = pool.clone();
        let sync_http = http.clone();
        let sync_url = config.endpoints.inaturalist_base_url.clone();
        let sync_token = config.inaturalist_api_token.clone();
        tokio::spawn(async move {
            match sync_naturalista_clade_species(
                &sync_pool,
                &sync_http,
                &sync_url,
                sync_token.as_deref(),
                &NaturaListaSyncOptions {
                    page: Some(1),
                    category: None,
                },
            )
            .await
            {
                Ok(report) => tracing::info!(
                    upserted = report.upserted_species,
                    filtered = report.filtered_non_species,
                    total = report.total_catalog_species,
                    "initial NaturaLista Collection sync completed"
                ),
                Err(err) => tracing::warn!(
                    error = %err,
                    "initial NaturaLista Collection sync skipped/deferred"
                ),
            }
        });
    } else {
        tracing::info!(
            naturalista_synced_count,
            "NaturaLista Collection catalog already populated"
        );
    }

    let vision: Arc<dyn TaxonomyClassifier> = match config.vision_provider {
        VisionProviderKind::INaturalist => Arc::new(INaturalistVisionClient::new(
            http.clone(),
            config.inaturalist_api_token.clone(),
            config.endpoints.inaturalist_base_url.clone(),
            config.vision_min_confidence,
            config.allow_dev_taxon_hint,
        )),
        VisionProviderKind::Mock => Arc::new(MockVisionClient::default_cached(
            config.vision_min_confidence,
        )),
    };

    tracing::info!(provider = ?config.vision_provider, "configured vision classifier");

    let state = AppState {
        db: pool,
        http,
        mods,
        vision,
        taxonomy_endpoints: config.endpoints,
        inaturalist_api_token: config.inaturalist_api_token,
        max_moire_score: config.max_moire_score,
        enable_live_range_fallback: config.enable_naturalista_range_fallback,
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
