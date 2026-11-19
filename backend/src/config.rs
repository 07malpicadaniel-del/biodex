//! Runtime configuration loaded from environment variables.

use std::env;
use std::path::PathBuf;

use thiserror::Error;

/// Default maximum allowed Moiré score for live camera captures.
pub const DEFAULT_MAX_MOIRE_SCORE: f32 = 0.35;

/// Default HTTP bind address (listens on all interfaces for local USB/LAN device testing).
pub const DEFAULT_BIND_ADDR: &str = "0.0.0.0:3000";

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
    /// Optional iNaturalist API JWT/token for computer vision species scoring.
    pub inaturalist_api_token: Option<String>,
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

        let max_moire_score = match env::var("MAX_MOIRE_SCORE") {
            Ok(raw) => {
                let parsed: f32 = raw.parse().map_err(|_| ConfigError::InvalidValue {
                    key: "MAX_MOIRE_SCORE",
                    reason: "must be a valid float".to_string(),
                })?;
                if !parsed.is_finite() || !(0.0..=1.0).contains(&parsed) {
                    return Err(ConfigError::InvalidValue {
                        key: "MAX_MOIRE_SCORE",
                        reason: "must be within 0.0..=1.0".to_string(),
                    });
                }
                parsed
            }
            Err(_) => DEFAULT_MAX_MOIRE_SCORE,
        };

        let inaturalist_api_token = env::var("INATURALIST_API_TOKEN")
            .ok()
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty());

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
            inaturalist_api_token,
            allow_dev_taxon_hint,
        })
    }
}
