//! OpenStreetMap Overpass API client and polygon normalizer for `geo_zones` ingestion.

use std::collections::BTreeMap;

use serde::Deserialize;
use sqlx::{PgPool, Row};
use thiserror::Error;

const COORD_EPSILON: f64 = 1e-7;

/// Parsed polygon ready for PostGIS `MultiPolygon` upsert into `geo_zones`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OsmGeoZone {
    /// OSM element type (`"way"` or `"relation"`).
    pub osm_type: &'static str,
    /// OSM numeric element identifier.
    pub osm_id: i64,
    /// Localized or fallback zone name.
    pub name: String,
    /// `"sanctuary"` or `"nature_reserve"`.
    pub zone_category: &'static str,
    /// `"zoo"`, `"aquarium"`, or `"nature_reserve"`.
    pub zone_type: &'static str,
    /// Valid WKT `MULTIPOLYGON(...)` string in `SRID 4326` (`lon lat`).
    pub wkt_multipolygon: String,
}

/// Summary counters returned by [`sync_geozones_from_overpass`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SyncSummary {
    /// Total valid polygons extracted from Overpass JSON.
    pub parsed_zones: usize,
    /// Number of rows inserted or updated in `geo_zones`.
    pub upserted_rows: usize,
    /// Number of elements skipped due to empty or non-polygonal geometry.
    pub skipped_elements: usize,
}

/// Errors produced while querying Overpass or ingesting PostGIS polygons.
#[derive(Debug, Error)]
pub enum OverpassError {
    /// Bounding box string did not match `south,west,north,east` bounds.
    #[error(
        "invalid bounding box '{0}' (expected 'south,west,north,east' with south<north and west<east)"
    )]
    InvalidBoundingBox(String),
    /// HTTP request to Overpass interpreter failed.
    #[error("overpass HTTP request failed: {0}")]
    Http(String),
    /// Overpass JSON response could not be parsed.
    #[error("overpass JSON parse error: {0}")]
    Parse(String),
    /// PostGIS upsert query failed.
    #[error("postgis upsert error: {0}")]
    Database(#[from] sqlx::Error),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Coord {
    lon: f64,
    lat: f64,
}

impl Coord {
    fn near(self, other: Self) -> bool {
        (self.lon - other.lon).abs() <= COORD_EPSILON
            && (self.lat - other.lat).abs() <= COORD_EPSILON
    }
}

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<OverpassElement>,
}

#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(rename = "type")]
    kind: String,
    id: i64,
    #[serde(default)]
    tags: BTreeMap<String, String>,
    #[serde(default)]
    geometry: Vec<OverpassPoint>,
    #[serde(default)]
    members: Vec<OverpassMember>,
}

#[derive(Debug, Deserialize)]
struct OverpassPoint {
    lat: f64,
    lon: f64,
}

#[derive(Debug, Deserialize)]
struct OverpassMember {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    geometry: Vec<OverpassPoint>,
}

/// Validates a `south,west,north,east` bounding box and builds the Overpass QL query
/// for zoos, aquariums, and nature reserves.
pub fn build_overpass_query(bbox: &str) -> Result<String, OverpassError> {
    let parts: Vec<f64> = bbox
        .split(',')
        .map(|s| s.trim().parse::<f64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| OverpassError::InvalidBoundingBox(bbox.to_string()))?;

    if parts.len() != 4 || !parts.iter().all(|v| v.is_finite()) {
        return Err(OverpassError::InvalidBoundingBox(bbox.to_string()));
    }

    let [south, west, north, east] = [parts[0], parts[1], parts[2], parts[3]];
    if !(-90.0..=90.0).contains(&south)
        || !(-90.0..=90.0).contains(&north)
        || !(-180.0..=180.0).contains(&west)
        || !(-180.0..=180.0).contains(&east)
        || south >= north
        || west >= east
    {
        return Err(OverpassError::InvalidBoundingBox(bbox.to_string()));
    }

    let formatted = format!("{south:.6},{west:.6},{north:.6},{east:.6}");
    Ok(format!(
        r#"[out:json][timeout:60];
(
  way["tourism"="zoo"]({formatted});
  relation["tourism"="zoo"]({formatted});
  way["tourism"="aquarium"]({formatted});
  relation["tourism"="aquarium"]({formatted});
  way["leisure"="nature_reserve"]({formatted});
  relation["leisure"="nature_reserve"]({formatted});
);
out geom;"#
    ))
}

/// Parses an Overpass API JSON payload into validated [`OsmGeoZone`] entries.
pub fn parse_overpass_elements(raw_json: &str) -> Result<(Vec<OsmGeoZone>, usize), OverpassError> {
    let parsed: OverpassResponse =
        serde_json::from_str(raw_json).map_err(|err| OverpassError::Parse(err.to_string()))?;

    let mut zones = Vec::new();
    let mut skipped = 0usize;

    for el in parsed.elements {
        let Some((zone_category, zone_type)) = classify_osm_tags(&el.tags) else {
            skipped += 1;
            continue;
        };

        let name = extract_zone_name(&el.tags, el.id);

        let result = match el.kind.as_str() {
            "way" => build_way_wkt(&el.geometry).map(|wkt| OsmGeoZone {
                osm_type: "way",
                osm_id: el.id,
                name,
                zone_category,
                zone_type,
                wkt_multipolygon: wkt,
            }),
            "relation" => build_relation_wkt(&el.members).map(|wkt| OsmGeoZone {
                osm_type: "relation",
                osm_id: el.id,
                name,
                zone_category,
                zone_type,
                wkt_multipolygon: wkt,
            }),
            _ => None,
        };

        match result {
            Some(zone) if zone.osm_id > 0 => zones.push(zone),
            _ => skipped += 1,
        }
    }

    Ok((zones, skipped))
}

/// Fetches OSM zones from Overpass and upserts them idempotently into `geo_zones`.
pub async fn sync_geozones_from_overpass(
    pool: &PgPool,
    http: &reqwest::Client,
    overpass_url: &str,
    bbox: &str,
    region_tag: &str,
    dry_run: bool,
) -> Result<SyncSummary, OverpassError> {
    let query = build_overpass_query(bbox)?;
    let resp = http
        .post(overpass_url)
        .form(&[("data", query.as_str())])
        .send()
        .await
        .map_err(|err| OverpassError::Http(err.to_string()))?;

    if !resp.status().is_success() {
        return Err(OverpassError::Http(format!(
            "overpass returned HTTP {}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|err| OverpassError::Http(err.to_string()))?;

    let (zones, mut skipped_elements) = parse_overpass_elements(&body)?;
    let parsed_zones = zones.len();

    if dry_run {
        return Ok(SyncSummary {
            parsed_zones,
            upserted_rows: 0,
            skipped_elements,
        });
    }

    let mut upserted_rows = 0usize;
    for zone in zones {
        let row = sqlx::query(
            r#"
            WITH candidate AS (
                SELECT ST_Multi(ST_CollectionExtract(ST_MakeValid(ST_GeomFromText($6, 4326)), 3)) AS geom
            )
            INSERT INTO geo_zones (
                osm_type, osm_id, name, zone_category, zone_type,
                boundary, region_tag, source_attribution, verified, updated_at
            )
            SELECT
                $1, $2, $3, $4, $5,
                geom, $7, 'OpenStreetMap / Overpass', TRUE, NOW()
            FROM candidate
            WHERE NOT ST_IsEmpty(geom)
            ON CONFLICT (osm_type, osm_id) DO UPDATE SET
                name = EXCLUDED.name,
                zone_category = EXCLUDED.zone_category,
                zone_type = EXCLUDED.zone_type,
                boundary = EXCLUDED.boundary,
                region_tag = EXCLUDED.region_tag,
                updated_at = NOW()
            RETURNING id
            "#,
        )
        .bind(zone.osm_type)
        .bind(zone.osm_id)
        .bind(&zone.name)
        .bind(zone.zone_category)
        .bind(zone.zone_type)
        .bind(&zone.wkt_multipolygon)
        .bind(region_tag)
        .fetch_optional(pool)
        .await?;

        if let Some(inserted) = row {
            let _id: uuid::Uuid = inserted.try_get("id")?;
            upserted_rows += 1;
        } else {
            skipped_elements += 1;
        }
    }

    Ok(SyncSummary {
        parsed_zones,
        upserted_rows,
        skipped_elements,
    })
}

fn classify_osm_tags(tags: &BTreeMap<String, String>) -> Option<(&'static str, &'static str)> {
    match tags.get("tourism").map(String::as_str) {
        Some("zoo") => return Some(("sanctuary", "zoo")),
        Some("aquarium") => return Some(("sanctuary", "aquarium")),
        _ => {}
    }
    if tags.get("leisure").map(String::as_str) == Some("nature_reserve") {
        return Some(("nature_reserve", "nature_reserve"));
    }
    None
}

fn extract_zone_name(tags: &BTreeMap<String, String>, osm_id: i64) -> String {
    tags.get("name:es")
        .or_else(|| tags.get("name"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| format!("Zona Natural OSM #{osm_id}"))
}

fn to_coords(points: &[OverpassPoint]) -> Option<Vec<Coord>> {
    let mut coords = Vec::with_capacity(points.len());
    for pt in points {
        if !pt.lat.is_finite()
            || !pt.lon.is_finite()
            || !(-90.0..=90.0).contains(&pt.lat)
            || !(-180.0..=180.0).contains(&pt.lon)
        {
            return None;
        }
        coords.push(Coord {
            lon: pt.lon,
            lat: pt.lat,
        });
    }
    Some(coords)
}

fn close_ring(mut ring: Vec<Coord>) -> Option<Vec<Coord>> {
    if ring.len() < 3 {
        return None;
    }
    if let (Some(first), Some(last)) = (ring.first().copied(), ring.last().copied())
        && !first.near(last)
    {
        ring.push(first);
    }
    if ring.len() < 4 {
        return None;
    }
    Some(ring)
}

fn build_way_wkt(points: &[OverpassPoint]) -> Option<String> {
    let ring = close_ring(to_coords(points)?)?;
    Some(format!("MULTIPOLYGON(({}))", format_ring(&ring)))
}

fn build_relation_wkt(members: &[OverpassMember]) -> Option<String> {
    let mut segments: Vec<Vec<Coord>> = Vec::new();
    for member in members {
        if member.kind == "way"
            && (member.role == "outer" || member.role.is_empty())
            && let Some(coords) = to_coords(&member.geometry)
            && coords.len() >= 2
        {
            segments.push(coords);
        }
    }

    let rings = stitch_segments_into_rings(segments);
    if rings.is_empty() {
        return None;
    }

    let polygons: Vec<String> = rings
        .into_iter()
        .map(|ring| format!("({})", format_ring(&ring)))
        .collect();
    Some(format!("MULTIPOLYGON({})", polygons.join(", ")))
}

fn stitch_segments_into_rings(mut segments: Vec<Vec<Coord>>) -> Vec<Vec<Coord>> {
    let mut rings = Vec::new();

    while !segments.is_empty() {
        let mut current = segments.remove(0);
        while let (Some(start), Some(end)) = (current.first().copied(), current.last().copied()) {
            if current.len() >= 4 && start.near(end) {
                if let Some(closed) = close_ring(current.clone()) {
                    rings.push(closed);
                }
                break;
            }

            let next_idx = segments.iter().position(|seg| {
                seg.first().is_some_and(|f| f.near(end)) || seg.last().is_some_and(|l| l.near(end))
            });

            let Some(idx) = next_idx else {
                // Single unclosed segment fallback if >= 3 points (rare malformed OSM way).
                if let Some(closed) = close_ring(current) {
                    rings.push(closed);
                }
                break;
            };

            let mut seg = segments.remove(idx);
            if seg.last().is_some_and(|l| l.near(end)) {
                seg.reverse();
            }
            current.extend(seg.into_iter().skip(1));
        }
    }

    rings
}

fn format_ring(ring: &[Coord]) -> String {
    let coords: Vec<String> = ring
        .iter()
        .map(|pt| format!("{:.7} {:.7}", pt.lon, pt.lat))
        .collect();
    format!("({})", coords.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_bbox_and_builds_overpass_query() {
        let q = build_overpass_query("17.10,-98.70,22.50,-93.60").unwrap();
        assert!(q.contains(r#"way["tourism"="zoo"](17.100000,-98.700000,22.500000,-93.600000);"#));
        assert!(q.contains(r#"relation["leisure"="nature_reserve"]"#));

        assert!(matches!(
            build_overpass_query("25.0,-98.0,20.0,-93.0"),
            Err(OverpassError::InvalidBoundingBox(_))
        ));
    }

    #[test]
    fn parses_closed_way_and_stitched_multipolygon_relation() {
        let json = r#"{
            "elements": [
                {
                    "type": "way",
                    "id": 1001,
                    "tags": { "tourism": "aquarium", "name": "Acuario de Veracruz" },
                    "geometry": [
                        { "lat": 19.190, "lon": -96.130 },
                        { "lat": 19.190, "lon": -96.125 },
                        { "lat": 19.195, "lon": -96.125 },
                        { "lat": 19.190, "lon": -96.130 }
                    ]
                },
                {
                    "type": "relation",
                    "id": 2002,
                    "tags": { "leisure": "nature_reserve", "name:es": "Reserva Los Tuxtlas" },
                    "members": [
                        {
                            "type": "way",
                            "role": "outer",
                            "geometry": [
                                { "lat": 18.500, "lon": -95.200 },
                                { "lat": 18.500, "lon": -95.100 },
                                { "lat": 18.600, "lon": -95.100 }
                            ]
                        },
                        {
                            "type": "way",
                            "role": "outer",
                            "geometry": [
                                { "lat": 18.600, "lon": -95.100 },
                                { "lat": 18.600, "lon": -95.200 },
                                { "lat": 18.500, "lon": -95.200 }
                            ]
                        }
                    ]
                },
                {
                    "type": "way",
                    "id": 3003,
                    "tags": { "tourism": "zoo", "name": "Too Short" },
                    "geometry": [
                        { "lat": 19.0, "lon": -96.0 },
                        { "lat": 19.1, "lon": -96.1 }
                    ]
                }
            ]
        }"#;

        let (zones, skipped) = parse_overpass_elements(json).unwrap();
        assert_eq!(zones.len(), 2);
        assert_eq!(skipped, 1);

        assert_eq!(zones[0].osm_type, "way");
        assert_eq!(zones[0].zone_category, "sanctuary");
        assert_eq!(zones[0].zone_type, "aquarium");
        assert_eq!(zones[0].name, "Acuario de Veracruz");
        assert!(zones[0].wkt_multipolygon.starts_with("MULTIPOLYGON((("));

        assert_eq!(zones[1].osm_type, "relation");
        assert_eq!(zones[1].zone_category, "nature_reserve");
        assert_eq!(zones[1].name, "Reserva Los Tuxtlas");
        assert!(zones[1].wkt_multipolygon.starts_with("MULTIPOLYGON((("));
    }
}
