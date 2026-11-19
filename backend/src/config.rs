//! Runtime configuration loaded from environment variables.

use std::env;
use std::path::PathBuf;

use thiserror::Error;

/// Default maximum allowed Moiré score for live camera captures.
pub const DEFAULT_MAX_MOIRE_SCORE: f32 = 0.35;

/// Default minimum vision confidence score (`0.0..=1.0`).
pub const DEFAULT_VISION_MIN_CONFIDENCE: f32 = 0.35;

/// Default HTTP bind address (listens on all interfaces for local USB/LAN device testing).
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";

/// Default iNaturalist API v1 base URL.
pub const DEFAULT_INATURALIST_TAXA_URL: &str = "https://api.inaturalist.org/v1";

/// Default GBIF API v1 base URL.
pub const DEFAULT_GBIF_API_URL: &str = "https://api.gbif.org/v1";

/// Default Wikidata MediaWiki API endpoint.
pub const DEFAULT_WIKIDATA_API_URL: &str = "https://www.wikidata.org/w/api.php";

/// Default OpenStreetMap Overpass interpreter endpoint.
pub const DEFAULT_OVERPASS_API_URL: &str = "https://overpass-api.de/api/interpreter";

/// Default bounding box covering Veracruz / central-east Mexico (`south,west,north,east`).
pub const DEFAULT_OVERPASS_BBOX: &str = "17.10,-98.70,22.50,-93.60";

/// Selectable taxonomic vision provider mode (`VISION_PROVIDER`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisionProviderKind {
    /// Cloud iNaturalist Computer Vision API + taxonomy rollup.
    INaturalist,
    /// Deterministic offline mock provider backed by cached iNaturalist JSON payloads.
    Mock,
}

/// Configuration errors during backend startup.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Required environment variable is missing.
    #[error("missing required environment variable '{0}'")]
    MissingEnv(&'static str),
    /// Environment variable value failed validation.
    #[error("invalid value for '{key}': {reason}")]
    InvalidValue {
        /// Variable name.
        key: &'static str,
        /// Validation failure description.
        reason: String,
    },
}

/// External API endpoints shared by vision normalization, biological enrichment, and geo sync.
#[derive(Debug, Clone)]
pub struct TaxonomyEndpoints {
    /// Base URL for iNaturalist API v1 (default `https://api.inaturalist.org/v1`).
    pub inaturalist_base_url: String,
    /// Base URL for GBIF API v1 (default `https://api.gbif.org/v1`).
    pub gbif_base_url: String,
    /// Endpoint URL for Wikidata API (default `https://www.wikidata.org/w/api.php`).
    pub wikidata_api_url: String,
}

impl Default for TaxonomyEndpoints {
    fn default() -> Self {
        Self {
            inaturalist_base_url: DEFAULT_INATURALIST_TAXA_URL.to_string(),
            gbif_base_url: DEFAULT_GBIF_API_URL.to_string(),
            wikidata_api_url: DEFAULT_WIKIDATA_API_URL.to_string(),
        }
    }
}

/// Runtime settings for the BioDex Phase 1 API server.
#[derive(Debug, Clone)]
pub struct BackendConfig {
    /// PostgreSQL / PostGIS connection string.
    pub database_url: String,
    /// Socket address to bind the Axum HTTP server.
    pub bind_addr: String,
    /// Directory containing declarative mod packs (e.g. `mods/`).
    pub mods_dir: PathBuf,
    /// Maximum allowed anti-Moiré screen detection score (`0.0..=1.0`).
    pub max_moire_score: f32,
    /// Vision provider (`inaturalist` or `mock`).
    pub vision_provider: VisionProviderKind,
    /// Minimum required vision confidence (`0.0..=1.0`).
    pub vision_min_confidence: f32,
    /// Optional iNaturalist API JWT/token for computer vision species scoring.
    pub inaturalist_api_token: Option<String>,
    /// External taxonomy/enrichment endpoints.
    pub endpoints: TaxonomyEndpoints,
    /// Overpass API endpoint used by `sync-geozones`.
    pub overpass_api_url: String,
    /// Default bounding box used by `sync-geozones`.
    pub overpass_default_bbox: String,
    /// Reserved optional Google AI key for future phases.
    pub google_ai_api_key: Option<String>,
    /// Allows explicit `dev_taxon_hint` fallback when no CV token is configured (dev/test only).
    pub allow_dev_taxon_hint: bool,
}

impl BackendConfig {
    /// Loads configuration from process environment variables (call `dotenvy::dotenv()` beforehand).
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| ConfigError::MissingEnv("DATABASE_URL"))?;
        let bind_addr = env::var("BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string());
        let mods_dir = PathBuf::from(env::var("MODS_DIR").unwrap_or_else(|_| "mods".to_string()));

        let max_moire_score = parse_unit_float("MAX_MOIRE_SCORE", DEFAULT_MAX_MOIRE_SCORE)?;
        let vision_min_confidence =
            parse_unit_float("VISION_MIN_CONFIDENCE", DEFAULT_VISION_MIN_CONFIDENCE)?;

        let vision_provider = match env::var("VISION_PROVIDER")
            .unwrap_or_else(|_| "inaturalist".to_string())
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "inaturalist" | "" => VisionProviderKind::INaturalist,
            "mock" => VisionProviderKind::Mock,
            other => {
                return Err(ConfigError::InvalidValue {
                    key: "VISION_PROVIDER",
                    reason: format!(
                        "unsupported provider '{other}' (expected 'inaturalist' or 'mock')"
                    ),
                });
            }
        };

        let inaturalist_api_token = optional_non_empty_env("INATURALIST_API_TOKEN");
        let google_ai_api_key = optional_non_empty_env("GOOGLE_AI_API_KEY");

        let endpoints = TaxonomyEndpoints {
            inaturalist_base_url: trim_trailing_slash(
                &env::var("INATURALIST_TAXA_URL")
                    .unwrap_or_else(|_| DEFAULT_INATURALIST_TAXA_URL.to_string()),
            ),
            gbif_base_url: trim_trailing_slash(
                &env::var("GBIF_API_URL").unwrap_or_else(|_| DEFAULT_GBIF_API_URL.to_string()),
            ),
            wikidata_api_url: env::var("WIKIDATA_API_URL")
                .unwrap_or_else(|_| DEFAULT_WIKIDATA_API_URL.to_string())
                .trim()
                .to_string(),
        };

        let overpass_api_url = env::var("OVERPASS_API_URL")
            .unwrap_or_else(|_| DEFAULT_OVERPASS_API_URL.to_string())
            .trim()
            .to_string();
        let overpass_default_bbox = env::var("OVERPASS_DEFAULT_BBOX")
            .unwrap_or_else(|_| DEFAULT_OVERPASS_BBOX.to_string())
            .trim()
            .to_string();

        let allow_dev_taxon_hint = matches!(
            env::var("ALLOW_DEV_TAXON_HINT")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes"
        );

        Ok(Self {
            database_url,
            bind_addr,
            mods_dir,
            max_moire_score,
            vision_provider,
            vision_min_confidence,
            inaturalist_api_token,
            endpoints,
            overpass_api_url,
            overpass_default_bbox,
            google_ai_api_key,
            allow_dev_taxon_hint,
        })
    }
}

fn parse_unit_float(key: &'static str, default_value: f32) -> Result<f32, ConfigError> {
    match env::var(key) {
        Ok(raw) if !raw.trim().is_empty() => {
            let parsed: f32 = raw.trim().parse().map_err(|_| ConfigError::InvalidValue {
                key,
                reason: "must be a valid float".to_string(),
            })?;
            if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                return Err(ConfigError::InvalidValue {
                    key,
                    reason: "must be within 0.0..=1.0".to_string(),
                });
            }
            Ok(parsed)
        }
        _ => Ok(default_value),
    }
}

fn optional_non_empty_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|val| val.trim().to_string())
        .filter(|val| !val.is_empty())
}

fn trim_trailing_slash(url: &str) -> String {
    url.trim().trim_end_matches('/').to_string()
}
