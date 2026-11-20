//! CLI binary to synchronize living wildlife species (no subspecies) from
//! `.env`'s iNaturalist / NaturaLista endpoint into PostgreSQL `species_catalog`.

use std::env;
use std::time::Duration;

use biodex_backend::almanac::{
    NaturaListaSyncOptions, sync_almanac_catalog, sync_naturalista_clade_species,
};
use biodex_backend::config::BackendConfig;
use biodex_backend::{load_mods_from_dir, run_migrations};
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("biodex_backend=info,sync_naturalista=info")),
        )
        .init();

    let config = BackendConfig::from_env()?;
    let (pages, category) = parse_cli_args()?;

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    run_migrations(&pool).await?;

    let mods = load_mods_from_dir(&config.mods_dir)?;
    let core_synced = sync_almanac_catalog(&pool, &mods).await?;
    tracing::info!(
        core_synced,
        "core almanac catalog verified before NaturaLista sync"
    );

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent("BioDex/0.1.0 (Open Source Citizen Science NaturaLista Sync)")
        .build()?;

    for page in 1..=pages {
        tracing::info!(
            page,
            category = ?category,
            inat_url = %config.endpoints.inaturalist_base_url,
            "synchronizing NaturaLista clades (species rank only)"
        );

        let report = sync_naturalista_clade_species(
            &pool,
            &http,
            &config.endpoints.inaturalist_base_url,
            config.inaturalist_api_token.as_deref(),
            &NaturaListaSyncOptions {
                page: Some(page),
                category: category.clone(),
            },
        )
        .await?;

        tracing::info!(
            page = report.page,
            fetched = report.fetched_candidates,
            filtered_non_species = report.filtered_non_species,
            upserted = report.upserted_species,
            total_catalog_species = report.total_catalog_species,
            "NaturaLista page synchronization finished"
        );
    }

    Ok(())
}

fn parse_cli_args() -> Result<(u32, Option<String>), String> {
    let mut pages: u32 = 1;
    let mut category: Option<String> = None;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--pages" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--pages requires a positive integer (1..=10)".to_string())?;
                pages = raw
                    .parse::<u32>()
                    .map_err(|_| format!("invalid --pages value '{raw}'"))?
                    .clamp(1, 10);
            }
            "--category" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--category requires a clade slug".to_string())?;
                category = Some(raw);
            }
            other => {
                return Err(format!(
                    "unknown argument '{other}' (supported: --pages <N>, --category <slug>)"
                ));
            }
        }
    }

    Ok((pages, category))
}
