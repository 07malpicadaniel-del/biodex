//! PostGIS spatial validation (`Sanctuary` vs. `Wild` vs. `OutOfRange`) and Overpass ingestion.

pub mod overpass;

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
    /// Takes priority over wild ranges and nature reserves if both overlap.
    Sanctuary {
        /// Matched `sanctuary_zones.id` or `geo_zones.id`.
        zone_id: Uuid,
        /// Registered sanctuary display name.
        zone_name: String,
        /// Sanctuary kind (`zoo`, `aquarium`, or `uma`).
        zone_type: String,
    },
    /// Coordinate falls inside the species' natural wild distribution range or a verified nature reserve.
    Wild,
    /// Coordinate falls outside any registered sanctuary, wild range, or nature reserve.
    OutOfRange,
}

/// Full spatial resolution including internal foreign keys for `captures` provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoResolution {
    /// Classification exposed in the HTTP response.
    pub classification: GeoClassification,
    /// Populated when matched against legacy/curated `sanctuary_zones`.
    pub sanctuary_zone_id: Option<Uuid>,
    /// Populated when matched against OSM/Overpass `geo_zones` (sanctuary or nature reserve).
    pub geo_zone_id: Option<Uuid>,
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

/// Classifies a capture coordinate against sanctuaries, species wild ranges, and OSM nature reserves.
///
/// Sanctuary polygons (`sanctuary_zones` + `geo_zones` sanctuaries) have strict priority over
/// wild ranges and nature reserves when boundaries overlap.
pub async fn classify_capture_location(
    pool: &PgPool,
    point: GeoPoint,
    taxon_id: TaxonId,
) -> Result<GeoResolution, GeoError> {
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

    // 1. Sanctuary priority across both curated `sanctuary_zones` and imported `geo_zones`.
    let sanctuary_row = sqlx::query(
        r#"
        SELECT id, name, zone_type, origin
        FROM (
            SELECT id, name, zone_type, 'sanctuary_zones' AS origin, 0 AS source_rank
            FROM sanctuary_zones
            WHERE verified = TRUE
              AND ST_Covers(boundary, ST_SetSRID(ST_MakePoint($1, $2), 4326))
            UNION ALL
            SELECT id, name, zone_type, 'geo_zones' AS origin, 1 AS source_rank
            FROM geo_zones
            WHERE verified = TRUE
              AND zone_category = 'sanctuary'
              AND ST_Covers(boundary, ST_SetSRID(ST_MakePoint($1, $2), 4326))
        ) matched_sanctuaries
        ORDER BY source_rank ASC, name ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(point.longitude)
    .bind(point.latitude)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = sanctuary_row {
        let zone_id: Uuid = row.try_get("id")?;
        let origin: String = row.try_get("origin")?;
        let (sanctuary_zone_id, geo_zone_id) = if origin == "geo_zones" {
            (None, Some(zone_id))
        } else {
            (Some(zone_id), None)
        };
        return Ok(GeoResolution {
            classification: GeoClassification::Sanctuary {
                zone_id,
                zone_name: row.try_get("name")?,
                zone_type: row.try_get("zone_type")?,
            },
            sanctuary_zone_id,
            geo_zone_id,
        });
    }

    // 2. Species-specific wild distribution polygons.
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
        return Ok(GeoResolution {
            classification: GeoClassification::Wild,
            sanctuary_zone_id: None,
            geo_zone_id: None,
        });
    }

    // 3. Verified OSM nature reserves (`leisure=nature_reserve`) grant Wild in Phase 1.
    let reserve_row = sqlx::query(
        r#"
        SELECT id
        FROM geo_zones
        WHERE verified = TRUE
          AND zone_category = 'nature_reserve'
          AND ST_Covers(boundary, ST_SetSRID(ST_MakePoint($1, $2), 4326))
        ORDER BY name ASC, id ASC
        LIMIT 1
        "#,
    )
    .bind(point.longitude)
    .bind(point.latitude)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = reserve_row {
        let reserve_id: Uuid = row.try_get("id")?;
        return Ok(GeoResolution {
            classification: GeoClassification::Wild,
            sanctuary_zone_id: None,
            geo_zone_id: Some(reserve_id),
        });
    }

    Ok(GeoResolution {
        classification: GeoClassification::OutOfRange,
        sanctuary_zone_id: None,
        geo_zone_id: None,
    })
}

#[derive(Debug, Deserialize)]
struct INatObservationsCountResponse {
    #[serde(default)]
    total_results: u64,
}

/// Extends [`classify_capture_location`] with a regional NaturaLista / iNaturalist occurrence
/// check (`150 km` radius) when a species has no curated polygons in `species_wild_ranges` yet.
///
/// Curated sanctuaries, explicit `species_wild_ranges`, and OSM nature reserves always take
/// precedence. Non-verifiable/captive observations are excluded by the iNaturalist API default.
pub async fn classify_capture_location_with_naturalista(
    pool: &PgPool,
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    point: GeoPoint,
    taxon_id: TaxonId,
    enable_live_range_fallback: bool,
) -> Result<GeoResolution, GeoError> {
    let postgis_res = classify_capture_location(pool, point, taxon_id).await?;
    if !matches!(postgis_res.classification, GeoClassification::OutOfRange)
        || !enable_live_range_fallback
    {
        return Ok(postgis_res);
    }

    let TaxonId::INaturalist(inat_id) = taxon_id else {
        return Ok(postgis_res);
    };

    let (taxon_source, taxon_external_id) = taxon_db_parts(taxon_id)?;
    let has_curated_wild_polygons: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM species_wild_ranges
            WHERE taxon_source = $1 AND taxon_external_id = $2
        )
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .fetch_one(pool)
    .await?;

    if has_curated_wild_polygons {
        return Ok(postgis_res);
    }

    let base = inaturalist_base_url.trim_end_matches('/');
    let url = format!("{base}/observations");
    let lat_s = point.latitude.to_string();
    let lng_s = point.longitude.to_string();
    let taxon_s = inat_id.to_string();

    let present_in_region = match http
        .get(&url)
        .query(&[
            ("taxon_id", taxon_s.as_str()),
            ("lat", lat_s.as_str()),
            ("lng", lng_s.as_str()),
            ("radius", "150"),
            ("per_page", "1"),
        ])
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.text().await {
            Ok(body) => parse_inaturalist_observation_presence(&body),
            Err(err) => {
                tracing::warn!(error = %err, "failed reading NaturaLista range check response");
                false
            }
        },
        Ok(resp) => {
            tracing::warn!(status = %resp.status(), "NaturaLista range check returned non-success");
            false
        }
        Err(err) => {
            tracing::warn!(error = %err, "NaturaLista range check HTTP call failed");
            false
        }
    };

    if present_in_region {
        Ok(GeoResolution {
            classification: GeoClassification::Wild,
            sanctuary_zone_id: None,
            geo_zone_id: None,
        })
    } else {
        Ok(postgis_res)
    }
}

/// Parses an iNaturalist `/v1/observations` JSON payload to detect whether at least one
/// verifiable observation exists in the queried radius.
#[must_use]
pub fn parse_inaturalist_observation_presence(raw_json: &str) -> bool {
    serde_json::from_str::<INatObservationsCountResponse>(raw_json)
        .is_ok_and(|parsed| parsed.total_results > 0)
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

    #[test]
    fn detects_naturalista_observation_presence_from_json() {
        assert!(parse_inaturalist_observation_presence(
            r#"{"total_results":382,"results":[]}"#
        ));
        assert!(!parse_inaturalist_observation_presence(
            r#"{"total_results":0,"results":[]}"#
        ));
        assert!(!parse_inaturalist_observation_presence("invalid-json"));
    }
}
