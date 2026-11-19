//! PostGIS spatial validation (`Sanctuary` vs. `Wild` vs. `OutOfRange`).

use biodex_core::crypto::GeoPoint;
use biodex_core::taxonomy::TaxonId;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;
use uuid::Uuid;

/// Spatial classification outcome for a verified capture coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "classification", rename_all = "snake_case")]
pub enum GeoClassification {
    /// Coordinate falls inside a verified zoo, aquarium, or UMA sanctuary polygon.
    /// Takes priority over wild ranges if both overlap.
    Sanctuary {
        /// Matched `sanctuary_zones.id`.
        zone_id: Uuid,
        /// Registered sanctuary display name.
        zone_name: String,
        /// Sanctuary kind (`zoo`, `aquarium`, or `uma`).
        zone_type: String,
    },
    /// Coordinate falls inside the species' natural wild distribution range.
    Wild,
    /// Coordinate falls outside any registered sanctuary and outside natural species range.
    OutOfRange,
}

/// Errors produced during PostGIS geospatial validation.
#[derive(Debug, Error)]
pub enum GeoError {
    /// Invalid WGS 84 coordinate passed to PostGIS query.
    #[error("invalid coordinates ({latitude}, {longitude})")]
    InvalidCoordinates {
        /// Latitude.
        latitude: f64,
        /// Longitude.
        longitude: f64,
    },
    /// Taxon external identifier could not fit into PostgreSQL `BIGINT`.
    #[error("taxon external id exceeds BIGINT range")]
    TaxonIdOverflow,
    /// Database query failure.
    #[error("postgis query failed: {0}")]
    Database(#[from] sqlx::Error),
}

/// Converts a [`TaxonId`] into its canonical database `(taxon_source, taxon_external_id)` tuple.
pub fn taxon_db_parts(taxon_id: TaxonId) -> Result<(&'static str, i64), GeoError> {
    match taxon_id {
        TaxonId::Gbif(id) => Ok((
            "gbif",
            i64::try_from(id).map_err(|_| GeoError::TaxonIdOverflow)?,
        )),
        TaxonId::INaturalist(id) => Ok((
            "i_naturalist",
            i64::try_from(id).map_err(|_| GeoError::TaxonIdOverflow)?,
        )),
    }
}

/// Parses database `(taxon_source, taxon_external_id)` columns back into [`TaxonId`].
#[must_use]
pub fn parse_taxon_parts(source: &str, external_id: i64) -> Option<TaxonId> {
    let id = u64::try_from(external_id).ok()?;
    match source {
        "gbif" => Some(TaxonId::Gbif(id)),
        "i_naturalist" => Some(TaxonId::INaturalist(id)),
        _ => None,
    }
}

/// Classifies a capture coordinate against verified sanctuary polygons and species wild ranges.
///
/// Sanctuary polygons have strict priority over natural wild ranges when both overlap.
pub async fn classify_capture_location(
    pool: &PgPool,
    point: GeoPoint,
    taxon_id: TaxonId,
) -> Result<GeoClassification, GeoError> {
    if !point.latitude.is_finite()
        || !point.longitude.is_finite()
        || !(-90.0..=90.0).contains(&point.latitude)
        || !(-180.0..=180.0).contains(&point.longitude)
    {
        return Err(GeoError::InvalidCoordinates {
            latitude: point.latitude,
            longitude: point.longitude,
        });
    }

    // Sanctuary priority: prevent farming 100% Wild cards inside zoos/aquariums located
    // within natural biomes. Note ST_MakePoint takes (longitude, latitude).
    let sanctuary_row = sqlx::query(
        r#"
        SELECT id, name, zone_type
        FROM sanctuary_zones
        WHERE verified = TRUE
          AND ST_Covers(boundary, ST_SetSRID(ST_MakePoint($1, $2), 4326))
        ORDER BY name ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(point.longitude)
    .bind(point.latitude)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = sanctuary_row {
        return Ok(GeoClassification::Sanctuary {
            zone_id: row.try_get("id")?,
            zone_name: row.try_get("name")?,
            zone_type: row.try_get("zone_type")?,
        });
    }

    let (taxon_source, taxon_external_id) = taxon_db_parts(taxon_id)?;
    let wild_match = sqlx::query(
        r#"
        SELECT 1
        FROM species_wild_ranges
        WHERE taxon_source = $1
          AND taxon_external_id = $2
          AND ST_Covers(range_geom, ST_SetSRID(ST_MakePoint($3, $4), 4326))
        LIMIT 1
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(point.longitude)
    .bind(point.latitude)
    .fetch_optional(pool)
    .await?;

    if wild_match.is_some() {
        return Ok(GeoClassification::Wild);
    }

    Ok(GeoClassification::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_taxon_id_to_and_from_db_parts() {
        assert_eq!(
            taxon_db_parts(TaxonId::Gbif(5_189_893)).unwrap(),
            ("gbif", 5_189_893)
        );
        assert_eq!(
            taxon_db_parts(TaxonId::INaturalist(14_824)).unwrap(),
            ("i_naturalist", 14_824)
        );
        assert_eq!(
            parse_taxon_parts("gbif", 5_189_893),
            Some(TaxonId::Gbif(5_189_893))
        );
        assert_eq!(
            parse_taxon_parts("i_naturalist", 14_824),
            Some(TaxonId::INaturalist(14_824))
        );
        assert_eq!(parse_taxon_parts("unknown", 1), None);
    }

    #[test]
    fn rejects_invalid_taxon_overflow_on_conversion() {
        let err = taxon_db_parts(TaxonId::Gbif(u64::MAX)).unwrap_err();
        assert!(matches!(err, GeoError::TaxonIdOverflow));
    }
}
