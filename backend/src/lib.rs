//! BioDex Backend API crate (Phase 1 MVP).
//!
//! Exposes Axum HTTP routes, PostGIS polygon classification, and taxonomic
//! vision/metadata orchestration on top of `biodex-core`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

pub mod almanac;
pub mod config;
pub mod error;
pub mod geo;
pub mod routes;
pub mod vision;

use std::fs;
use std::path::Path;

use biodex_core::mods::{ModError, ModRegistry};

/// Runs embedded SQLx PostGIS migrations against the provided PostgreSQL pool.
pub async fn run_migrations(pool: &sqlx::PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}

/// Loads all valid mod packs found as immediate subdirectories inside `mods_dir`
/// (e.g., `mods/base-pack/`).
pub fn load_mods_from_dir(mods_dir: &Path) -> Result<ModRegistry, ModError> {
    let mut registry = ModRegistry::new();
    if !mods_dir.exists() {
        return Ok(registry);
    }

    let mut entries = fs::read_dir(mods_dir)
        .map_err(|err| ModError::Io(err.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| ModError::Io(err.to_string()))?;

    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() && path.join("mod.toml").is_file() {
            registry.load_dir(&path)?;
        }
    }

    Ok(registry)
}
