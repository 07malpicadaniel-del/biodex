//! CLI binary to import zoos, aquariums, and nature reserves from OSM Overpass into PostGIS.

use std::env;
use std::time::Duration;

use biodex_backend::config::BackendConfig;
use biodex_backend::geo::overpass::sync_geozones_from_overpass;
use biodex_backend::run_migrations;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("biodex_backend=info,sync_geozones=info")),
        )
        .init();

    let config = BackendConfig::from_env()?;
    let (bbox, region_tag, dry_run) = parse_cli_args(&config.overpass_default_bbox)?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    run_migrations(&pool).await?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(75))
        .user_agent("BioDex/0.1.0 (Open Source Citizen Science Geozones Sync)")
        .build()?;

    tracing::info!(
        overpass_url = %config.overpass_api_url,
        bbox = %bbox,
        region = %region_tag,
        dry_run,
        "querying Overpass API for geo_zones"
    );

    let summary = sync_geozones_from_overpass(
        &pool,
        &http,
        &config.overpass_api_url,
        &bbox,
        &region_tag,
        dry_run,
    )
    .await?;

    tracing::info!(
        parsed = summary.parsed_zones,
        upserted = summary.upserted_rows,
        skipped = summary.skipped_elements,
        "Overpass geo_zones synchronization finished"
    );

    Ok(())
}

fn parse_cli_args(default_bbox: &str) -> Result<(String, String, bool), String> {
    let mut bbox = default_bbox.to_string();
    let mut region = "veracruz".to_string();
    let mut dry_run = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bbox" => {
                bbox = args
                    .next()
                    .ok_or_else(|| "--bbox requires 'south,west,north,east'".to_string())?;
            }
            "--region" => {
                region = args
                    .next()
                    .ok_or_else(|| "--region requires a region tag".to_string())?;
            }
            "--dry-run" => dry_run = true,
            other => {
                return Err(format!(
                    "unknown argument '{other}' (supported: --bbox, --region, --dry-run)"
                ));
            }
        }
    }

    Ok((bbox, region, dry_run))
}
