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
    let cli = parse_cli_args()?;

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

    let end_page = (cli.start_page + cli.pages - 1).min(50);
    for page in cli.start_page..=end_page {
        tracing::info!(
            page,
            category = ?cli.category,
            backend_only = cli.backend_only,
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
                category: cli.category.clone(),
                backend_only: Some(cli.backend_only),
            },
        )
        .await?;

        tracing::info!(
            page = report.page,
            fetched = report.fetched_candidates,
            filtered_non_species = report.filtered_non_species,
            upserted = report.upserted_species,
            total_catalog_species = report.total_catalog_species,
            total_backend_indexed = report.total_backend_indexed,
            "NaturaLista page synchronization finished"
        );
    }

    Ok(())
}

struct CliArgs {
    start_page: u32,
    pages: u32,
    category: Option<String>,
    backend_only: bool,
}

fn parse_cli_args() -> Result<CliArgs, String> {
    let mut start_page: u32 = 1;
    let mut pages: u32 = 1;
    let mut category: Option<String> = None;
    let mut backend_only = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--start-page" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--start-page requires a positive integer".to_string())?;
                start_page = raw
                    .parse::<u32>()
                    .map_err(|_| format!("invalid --start-page value '{raw}'"))?
                    .clamp(1, 50);
            }
            "--pages" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--pages requires a positive integer (1..=20)".to_string())?;
                pages = raw
                    .parse::<u32>()
                    .map_err(|_| format!("invalid --pages value '{raw}'"))?
                    .clamp(1, 20);
            }
            "--category" => {
                let raw = args
                    .next()
                    .ok_or_else(|| "--category requires a clade slug".to_string())?;
                category = Some(raw);
            }
            "--backend-only" => backend_only = true,
            other => {
                return Err(format!(
                    "unknown argument '{other}' (supported: --start-page <N>, --pages <N>, --category <slug>, --backend-only)"
                ));
            }
        }
    }

    Ok(CliArgs {
        start_page,
        pages,
        category,
        backend_only,
    })
}
