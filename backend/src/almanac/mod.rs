//! Almanac (encyclopedia collection), NaturaLista media sync, and live species exploration service.

pub mod naturalista_sync;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use biodex_core::crypto::GeoPoint;
use biodex_core::mods::ModRegistry;
use biodex_core::taxonomy::{
    BiologicalTraits, CaptureVariant, DefenseTrait, Diet, Habitat, IucnStatus, SpeciesCard,
    TaxonId, TaxonomyError, generate_card,
};
use chrono::{DateTime, Utc};
pub use naturalista_sync::{
    NaturaListaSyncOptions, NaturaListaSyncReport, sync_naturalista_clade_species,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::config::TaxonomyEndpoints;
use crate::geo::{GeoError, parse_taxon_parts, taxon_db_parts};
use crate::vision::{
    INatTaxaResponse, INatTaxon, TaxonMediaMeta, TaxonomicRank, VisionError, VisionIdentification,
    clean_opt, extract_inat_media, is_non_fauna_taxon, naturalista_url_for_taxon, parse_diet,
    parse_habitat, parse_iucn, resolve_species_profile, serialize_slug,
};

/// Query parameters for `GET /api/v1/almanac`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AlmanacFilter {
    /// Case-insensitive substring search against common name, scientific name, or summary.
    pub q: Option<String>,
    /// Exact category filter (`canids`, `felines`, `primates`, `mammals`, `birds`,
    /// `reptiles_amphibians`, `sharks`, `invertebrates`, `insects_arachnids`, `aquatic`).
    pub category: Option<String>,
    /// Exact habitat filter (`terrestrial`, `aerial`, `freshwater`, `marine`, `abyssal`).
    pub habitat: Option<String>,
    /// Optional player identifier used to compute `discovered`, `best_variant`, and `capture_count`.
    pub user_id: Option<String>,
    /// When `true` (Dev Mode login), unlocks every cataloged species for full-collection preview.
    #[serde(default)]
    pub dev_mode: bool,
    /// Optional discovery filter (`captured` or `missing`).
    pub captured_filter: Option<String>,
    /// Include NaturaLista-synced species alongside curated core Almanac species (`true` by default).
    pub include_naturalista: Option<bool>,
    /// Maximum number of entries returned in this page (returns all matching entries when omitted).
    pub limit: Option<i64>,
    /// Zero-based pagination offset (defaults to `0`).
    pub offset: Option<i64>,
}

/// Complete encyclopedia collection entry enriched with NaturaLista imagery and capture badge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlmanacItemDto {
    /// External provider taxon identifier.
    pub taxon_id: TaxonId,
    /// Binomial scientific name.
    pub scientific_name: String,
    /// Localized vernacular name.
    pub common_name: Option<String>,
    /// Taxonomic phylum.
    pub phylum: String,
    /// Taxonomic class.
    pub taxonomic_class: String,
    /// Average adult body mass in grams.
    pub average_mass_grams: u64,
    /// Primary diet.
    pub diet: Diet,
    /// Primary habitat.
    pub habitat: Habitat,
    /// Natural defense mechanisms.
    pub defenses: BTreeSet<DefenseTrait>,
    /// IUCN Red List conservation status.
    pub iucn_status: IucnStatus,
    /// Almanac grouping category.
    pub category: String,
    /// Concise encyclopedic summary.
    pub summary: String,
    /// Optional region or environment hint.
    pub region_hint: Option<String>,
    /// Baseline Wild card preview (with any loaded mod overrides applied).
    pub base_card: SpeciesCard,
    /// `true` when the player captured this species (or `dev_mode = true`).
    pub discovered: bool,
    /// Highest-tier variant captured (`"wild"` > `"sanctuary"`), or `None` if undiscovered.
    pub best_variant: Option<String>,
    /// Total non-discarded captures recorded for this species.
    pub capture_count: i64,
    /// Wildlife reference image URL sourced from NaturaLista / iNaturalist.
    #[serde(default)]
    pub image_url: Option<String>,
    /// Photographer / CC license attribution for `image_url`.
    #[serde(default)]
    pub photo_attribution: Option<String>,
    /// Official NaturaLista / iNaturalist or GBIF taxon page URL.
    #[serde(default)]
    pub naturalista_url: String,
    /// Optional Wikipedia encyclopedia URL.
    #[serde(default)]
    pub wikipedia_url: Option<String>,
    /// Global observation count reported on NaturaLista / iNaturalist.
    #[serde(default)]
    pub observations_count: Option<i64>,
    /// Whether this species belongs to the curated `base-pack` dictionary.
    #[serde(default)]
    pub is_core_almanac: bool,
    /// Most recent non-discarded capture timestamp for this user.
    #[serde(default)]
    pub last_captured_at_utc: Option<DateTime<Utc>>,
    /// Verified PostGIS zone name (zoo/reserve) of the latest capture, when applicable.
    #[serde(default)]
    pub last_zone_name: Option<String>,
}

/// Response returned by `GET /api/v1/almanac`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlmanacListResponse {
    /// Total number of collection species included in this view.
    pub total_species: usize,
    /// Number of species unlocked by captures (or all of them in `dev_mode`).
    pub discovered_count: usize,
    /// Total number of entries matching the active search/category/status filters before pagination.
    #[serde(default)]
    pub matched_count: usize,
    /// Per-clade species totals across the active base scope (`canids`, `felines`, `sharks`, etc.).
    #[serde(default)]
    pub category_counts: BTreeMap<String, usize>,
    /// Filtered and paginated list of Almanac/Collection entries.
    pub entries: Vec<AlmanacItemDto>,
}

/// Fauna entry returned by live NaturaLista search or nearby wildlife endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturaListaSpeciesDto {
    /// External provider taxon identifier (`INaturalist(id)`).
    pub taxon_id: TaxonId,
    /// Binomial scientific name.
    pub scientific_name: String,
    /// Preferred Spanish common name when available.
    pub common_name: Option<String>,
    /// Taxonomic iconic class/group.
    pub iconic_taxon_name: Option<String>,
    /// IUCN conservation status code, if known.
    pub iucn_status: IucnStatus,
    /// Thumbnail / medium photograph URL from NaturaLista.
    pub image_url: Option<String>,
    /// Photographer / CC attribution.
    pub photo_attribution: Option<String>,
    /// Canonical NaturaLista URL (`https://www.naturalista.mx/taxa/{id}`).
    pub naturalista_url: String,
    /// Optional Wikipedia article URL.
    pub wikipedia_url: Option<String>,
    /// Total global verifiable observations on NaturaLista / iNaturalist.
    pub global_observations_count: Option<i64>,
    /// Observations recorded within the queried GPS radius (nearby endpoint only).
    pub local_observations_count: Option<i64>,
    /// `true` if captured by the active player (or `dev_mode = true`).
    pub discovered: bool,
    /// Best captured variant (`"wild"` / `"sanctuary"`), if any.
    pub best_variant: Option<String>,
    /// Whether this species is already cached in `species_catalog`.
    pub in_catalog: bool,
}

/// Response envelope for live NaturaLista search and nearby observation endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturaListaDiscoveryResponse {
    /// Official NaturaLista portal link matching this query.
    pub source_url: String,
    /// Human-readable summary of the executed lookup.
    pub query_summary: String,
    /// Returned fauna species sorted by relevance or local abundance.
    pub entries: Vec<NaturaListaSpeciesDto>,
}

/// Errors produced during Almanac sync, query, or NaturaLista exploration.
#[derive(Debug, Error)]
pub enum AlmanacError {
    /// Invalid taxon ID conversion.
    #[error("geo/taxon error: {0}")]
    Geo(#[from] GeoError),
    /// Invalid biological traits when computing preview card.
    #[error("card generation failed: {0}")]
    Taxonomy(#[from] TaxonomyError),
    /// Invalid input parameter.
    #[error("invalid query parameter: {0}")]
    InvalidInput(String),
    /// Upstream NaturaLista API lookup failed.
    #[error("NaturaLista lookup failed: {0}")]
    Vision(#[from] VisionError),
    /// Database error while syncing or querying `species_catalog`.
    #[error("almanac database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Synchronizes all Almanac entries loaded in [`ModRegistry`] into PostgreSQL `species_catalog`.
pub async fn sync_almanac_catalog(
    pool: &PgPool,
    mods: &ModRegistry,
) -> Result<usize, AlmanacError> {
    let entries = mods.almanac_entries();
    for entry in &entries {
        let (taxon_source, taxon_external_id) = taxon_db_parts(entry.traits.taxon_id)?;
        let diet = serialize_slug(&entry.traits.diet);
        let habitat = serialize_slug(&entry.traits.habitat);
        let iucn = serialize_slug(&entry.traits.iucn_status);
        let defenses = serde_json::to_value(&entry.traits.defenses)
            .unwrap_or(serde_json::Value::Array(vec![]));
        let mass = i64::try_from(entry.traits.average_mass_grams).unwrap_or(i64::MAX);

        sqlx::query(
            r#"
            INSERT INTO species_catalog (
                taxon_source, taxon_external_id, scientific_name, common_name,
                phylum, taxonomic_class, average_mass_grams, diet, habitat, defenses,
                iucn_status, summary, category, region_hint, is_core_almanac, source_updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, TRUE, NOW())
            ON CONFLICT (taxon_source, taxon_external_id) DO UPDATE SET
                scientific_name = EXCLUDED.scientific_name,
                common_name = EXCLUDED.common_name,
                phylum = EXCLUDED.phylum,
                taxonomic_class = EXCLUDED.taxonomic_class,
                average_mass_grams = EXCLUDED.average_mass_grams,
                diet = EXCLUDED.diet,
                habitat = EXCLUDED.habitat,
                defenses = EXCLUDED.defenses,
                iucn_status = EXCLUDED.iucn_status,
                summary = EXCLUDED.summary,
                category = EXCLUDED.category,
                region_hint = EXCLUDED.region_hint,
                is_core_almanac = TRUE,
                source_updated_at = NOW()
            "#,
        )
        .bind(taxon_source)
        .bind(taxon_external_id)
        .bind(&entry.traits.scientific_name)
        .bind(&entry.traits.common_name)
        .bind(&entry.traits.phylum)
        .bind(&entry.traits.taxonomic_class)
        .bind(mass)
        .bind(diet)
        .bind(habitat)
        .bind(defenses)
        .bind(iucn)
        .bind(&entry.summary)
        .bind(entry.category.as_str())
        .bind(&entry.region_hint)
        .execute(pool)
        .await?;
    }

    Ok(entries.len())
}

/// Fetches missing public photographs and metadata from NaturaLista (`INATURALIST_TAXA_URL/taxa`)
/// for cataloged species in batches of up to 30 taxa.
pub async fn sync_almanac_media(
    pool: &PgPool,
    http: &reqwest::Client,
    inaturalist_base_url: &str,
) -> Result<usize, AlmanacError> {
    let rows = sqlx::query(
        r#"
        SELECT taxon_external_id
        FROM species_catalog
        WHERE taxon_source = 'i_naturalist'
          AND (image_url IS NULL OR naturalista_observations_count IS NULL)
        ORDER BY is_core_almanac DESC, taxon_external_id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut ids: Vec<i64> = Vec::with_capacity(rows.len());
    for row in rows {
        ids.push(row.try_get("taxon_external_id")?);
    }
    if ids.is_empty() {
        return Ok(0);
    }

    let base = inaturalist_base_url.trim_end_matches('/');
    let mut updated = 0usize;

    for chunk in ids.chunks(30) {
        let joined = chunk
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = format!("{base}/taxa");
        let resp = http
            .get(&url)
            .query(&[
                ("id", joined.as_str()),
                ("locale", "es"),
                ("per_page", "30"),
            ])
            .send()
            .await
            .map_err(|err| VisionError::Upstream(err.to_string()))?;

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "NaturaLista batch media sync failed");
            continue;
        }
        let body = resp
            .text()
            .await
            .map_err(|err| VisionError::Upstream(err.to_string()))?;
        let parsed: INatTaxaResponse =
            serde_json::from_str(&body).map_err(|err| VisionError::Upstream(err.to_string()))?;

        for taxon in &parsed.results {
            let media = extract_inat_media(taxon);
            let Ok(ext_id) = i64::try_from(taxon.id) else {
                continue;
            };
            sqlx::query(
                r#"
                UPDATE species_catalog
                SET image_url = COALESCE(image_url, $2),
                    photo_attribution = COALESCE(photo_attribution, $3),
                    wikipedia_url = COALESCE(wikipedia_url, $4),
                    naturalista_observations_count = COALESCE($5, naturalista_observations_count)
                WHERE taxon_source = 'i_naturalist' AND taxon_external_id = $1
                "#,
            )
            .bind(ext_id)
            .bind(&media.image_url)
            .bind(&media.photo_attribution)
            .bind(&media.wikipedia_url)
            .bind(media.observations_count)
            .execute(pool)
            .await?;
            updated += 1;
        }
    }

    Ok(updated)
}

/// Lists Almanac & captured collection species with discovery badges, photos, NaturaLista URLs,
/// server-side filtering, clade counts, and optional SQL pagination (`limit` / `offset`).
pub async fn list_almanac(
    pool: &PgPool,
    mods: &ModRegistry,
    filter: &AlmanacFilter,
) -> Result<AlmanacListResponse, AlmanacError> {
    let include_naturalista = filter.include_naturalista.unwrap_or(true);
    let user_id = filter.user_id.as_deref();

    let stats_rows = sqlx::query(
        r#"
        SELECT
            LOWER(sc.category) AS category,
            COUNT(*)::BIGINT AS total_count,
            COUNT(*) FILTER (WHERE COALESCE(cap.capture_count, 0) > 0)::BIGINT AS discovered_count
        FROM species_catalog sc
        LEFT JOIN LATERAL (
            SELECT COUNT(*) AS capture_count
            FROM captures c
            WHERE c.taxon_source = sc.taxon_source
              AND c.taxon_external_id = sc.taxon_external_id
              AND c.zone_classification IN ('wild', 'sanctuary')
              AND ($1::TEXT IS NULL OR c.user_id = $1)
        ) cap ON TRUE
        WHERE sc.taxonomic_rank = 'species'
          AND (
              sc.is_core_almanac = TRUE
              OR ($2::BOOL AND sc.is_naturalista_synced = TRUE)
              OR ($1::TEXT IS NOT NULL AND COALESCE(cap.capture_count, 0) > 0)
          )
        GROUP BY LOWER(sc.category)
        "#,
    )
    .bind(user_id)
    .bind(include_naturalista)
    .fetch_all(pool)
    .await?;

    let mut category_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_species = 0_usize;
    let mut real_discovered_count = 0_usize;
    for row in stats_rows {
        let cat: String = row.try_get("category")?;
        let cat_total = usize::try_from(row.try_get::<i64, _>("total_count")?.max(0)).unwrap_or(0);
        let cat_disc =
            usize::try_from(row.try_get::<i64, _>("discovered_count")?.max(0)).unwrap_or(0);
        total_species = total_species.saturating_add(cat_total);
        real_discovered_count = real_discovered_count.saturating_add(cat_disc);
        category_counts.insert(cat, cat_total);
    }
    let discovered_count = if filter.dev_mode {
        total_species
    } else {
        real_discovered_count
    };

    let search_pattern = filter
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(|q| format!("%{}%", escape_ilike_pattern(q)));
    let category_filter = filter
        .category
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .map(str::to_lowercase);
    let habitat_filter = filter
        .habitat
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(str::to_lowercase);
    let captured_mode: Option<&str> = match filter
        .captured_filter
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .as_deref()
    {
        Some("captured") => (!filter.dev_mode).then_some("captured"),
        Some("missing") => Some(if filter.dev_mode { "none" } else { "missing" }),
        _ => None,
    };

    let limit_val = filter.limit.map(|l| l.clamp(1, 500));
    let offset_val = filter.offset.unwrap_or(0).max(0);

    let rows = sqlx::query(
        r#"
        SELECT
            sc.taxon_source,
            sc.taxon_external_id,
            sc.scientific_name,
            sc.common_name,
            sc.phylum,
            sc.taxonomic_class,
            sc.average_mass_grams,
            sc.diet,
            sc.habitat,
            sc.defenses,
            sc.iucn_status,
            sc.category,
            sc.summary,
            sc.region_hint,
            sc.is_core_almanac,
            sc.image_url,
            sc.photo_attribution,
            sc.wikipedia_url,
            sc.naturalista_observations_count,
            COALESCE(cap.capture_count, 0) AS capture_count,
            COALESCE(cap.has_wild, FALSE) AS has_wild,
            COALESCE(cap.has_sanctuary, FALSE) AS has_sanctuary,
            cap.last_captured_at_utc,
            cap.last_zone_name,
            COUNT(*) OVER()::BIGINT AS matched_total
        FROM species_catalog sc
        LEFT JOIN LATERAL (
            SELECT
                COUNT(*) AS capture_count,
                BOOL_OR(c.zone_classification = 'wild') AS has_wild,
                BOOL_OR(c.zone_classification = 'sanctuary') AS has_sanctuary,
                MAX(c.captured_at_utc) AS last_captured_at_utc,
                (
                    SELECT COALESCE(sz.name, gz.name)
                    FROM captures c2
                    LEFT JOIN sanctuary_zones sz ON sz.id = c2.sanctuary_zone_id
                    LEFT JOIN geo_zones gz ON gz.id = c2.geo_zone_id
                    WHERE c2.taxon_source = sc.taxon_source
                      AND c2.taxon_external_id = sc.taxon_external_id
                      AND c2.zone_classification IN ('wild', 'sanctuary')
                      AND ($1::TEXT IS NULL OR c2.user_id = $1)
                    ORDER BY c2.captured_at_utc DESC
                    LIMIT 1
                ) AS last_zone_name
            FROM captures c
            WHERE c.taxon_source = sc.taxon_source
              AND c.taxon_external_id = sc.taxon_external_id
              AND c.zone_classification IN ('wild', 'sanctuary')
              AND ($1::TEXT IS NULL OR c.user_id = $1)
        ) cap ON TRUE
        WHERE sc.taxonomic_rank = 'species'
          AND (
              sc.is_core_almanac = TRUE
              OR ($2::BOOL AND sc.is_naturalista_synced = TRUE)
              OR ($1::TEXT IS NOT NULL AND COALESCE(cap.capture_count, 0) > 0)
          )
          AND ($3::TEXT IS NULL OR LOWER(sc.category) = $3)
          AND ($4::TEXT IS NULL OR LOWER(sc.habitat) = $4)
          AND (
              $5::TEXT IS NULL
              OR sc.scientific_name ILIKE $5 ESCAPE '\'
              OR COALESCE(sc.common_name, '') ILIKE $5 ESCAPE '\'
              OR sc.summary ILIKE $5 ESCAPE '\'
          )
          AND (
              $6::TEXT IS NULL
              OR ($6 = 'captured' AND COALESCE(cap.capture_count, 0) > 0)
              OR ($6 = 'missing' AND COALESCE(cap.capture_count, 0) = 0)
              OR ($6 = 'none' AND FALSE)
          )
        ORDER BY
            sc.is_core_almanac DESC,
            sc.naturalista_observations_count DESC NULLS LAST,
            COALESCE(sc.common_name, sc.scientific_name) ASC
        LIMIT $7
        OFFSET $8
        "#,
    )
    .bind(user_id)
    .bind(include_naturalista)
    .bind(category_filter.as_deref())
    .bind(habitat_filter.as_deref())
    .bind(search_pattern.as_deref())
    .bind(captured_mode)
    .bind(limit_val)
    .bind(offset_val)
    .fetch_all(pool)
    .await?;

    let matched_count = if let Some(first) = rows.first() {
        usize::try_from(first.try_get::<i64, _>("matched_total")?.max(0)).unwrap_or(0)
    } else if offset_val > 0 {
        let count_row = sqlx::query(
            r#"
            SELECT COUNT(*)::BIGINT AS matched_total
            FROM species_catalog sc
            LEFT JOIN LATERAL (
                SELECT COUNT(*) AS capture_count
                FROM captures c
                WHERE c.taxon_source = sc.taxon_source
                  AND c.taxon_external_id = sc.taxon_external_id
                  AND c.zone_classification IN ('wild', 'sanctuary')
                  AND ($1::TEXT IS NULL OR c.user_id = $1)
            ) cap ON TRUE
            WHERE sc.taxonomic_rank = 'species'
              AND (
                  sc.is_core_almanac = TRUE
                  OR ($2::BOOL AND sc.is_naturalista_synced = TRUE)
                  OR ($1::TEXT IS NOT NULL AND COALESCE(cap.capture_count, 0) > 0)
              )
              AND ($3::TEXT IS NULL OR LOWER(sc.category) = $3)
              AND ($4::TEXT IS NULL OR LOWER(sc.habitat) = $4)
              AND (
                  $5::TEXT IS NULL
                  OR sc.scientific_name ILIKE $5 ESCAPE '\'
                  OR COALESCE(sc.common_name, '') ILIKE $5 ESCAPE '\'
                  OR sc.summary ILIKE $5 ESCAPE '\'
              )
              AND (
                  $6::TEXT IS NULL
                  OR ($6 = 'captured' AND COALESCE(cap.capture_count, 0) > 0)
                  OR ($6 = 'missing' AND COALESCE(cap.capture_count, 0) = 0)
                  OR ($6 = 'none' AND FALSE)
              )
            "#,
        )
        .bind(user_id)
        .bind(include_naturalista)
        .bind(category_filter.as_deref())
        .bind(habitat_filter.as_deref())
        .bind(search_pattern.as_deref())
        .bind(captured_mode)
        .fetch_one(pool)
        .await?;
        usize::try_from(count_row.try_get::<i64, _>("matched_total")?.max(0)).unwrap_or(0)
    } else {
        0
    };

    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(item) = map_almanac_row(&row, mods, filter.dev_mode)? {
            entries.push(item);
        }
    }

    Ok(AlmanacListResponse {
        total_species,
        discovered_count,
        matched_count,
        category_counts,
        entries,
    })
}

fn escape_ilike_pattern(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' | '%' | '_' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Fetches a single Almanac/Collection entry by [`TaxonId`] with optional Dev Mode unlock.
pub async fn get_almanac_entry(
    pool: &PgPool,
    mods: &ModRegistry,
    taxon_id: TaxonId,
    user_id: Option<&str>,
    dev_mode: bool,
) -> Result<Option<AlmanacItemDto>, AlmanacError> {
    let (taxon_source, taxon_external_id) = taxon_db_parts(taxon_id)?;

    let row = sqlx::query(
        r#"
        SELECT
            sc.taxon_source,
            sc.taxon_external_id,
            sc.scientific_name,
            sc.common_name,
            sc.phylum,
            sc.taxonomic_class,
            sc.average_mass_grams,
            sc.diet,
            sc.habitat,
            sc.defenses,
            sc.iucn_status,
            sc.category,
            sc.summary,
            sc.region_hint,
            sc.is_core_almanac,
            sc.image_url,
            sc.photo_attribution,
            sc.wikipedia_url,
            sc.naturalista_observations_count,
            COALESCE(cap.capture_count, 0) AS capture_count,
            COALESCE(cap.has_wild, FALSE) AS has_wild,
            COALESCE(cap.has_sanctuary, FALSE) AS has_sanctuary,
            cap.last_captured_at_utc,
            cap.last_zone_name
        FROM species_catalog sc
        LEFT JOIN LATERAL (
            SELECT
                COUNT(*) AS capture_count,
                BOOL_OR(c.zone_classification = 'wild') AS has_wild,
                BOOL_OR(c.zone_classification = 'sanctuary') AS has_sanctuary,
                MAX(c.captured_at_utc) AS last_captured_at_utc,
                (
                    SELECT COALESCE(sz.name, gz.name)
                    FROM captures c2
                    LEFT JOIN sanctuary_zones sz ON sz.id = c2.sanctuary_zone_id
                    LEFT JOIN geo_zones gz ON gz.id = c2.geo_zone_id
                    WHERE c2.taxon_source = sc.taxon_source
                      AND c2.taxon_external_id = sc.taxon_external_id
                      AND c2.zone_classification IN ('wild', 'sanctuary')
                      AND ($3::TEXT IS NULL OR c2.user_id = $3)
                    ORDER BY c2.captured_at_utc DESC
                    LIMIT 1
                ) AS last_zone_name
            FROM captures c
            WHERE c.taxon_source = sc.taxon_source
              AND c.taxon_external_id = sc.taxon_external_id
              AND c.zone_classification IN ('wild', 'sanctuary')
              AND ($3::TEXT IS NULL OR c.user_id = $3)
        ) cap ON TRUE
        WHERE sc.taxon_source = $1 AND sc.taxon_external_id = $2
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => map_almanac_row(&r, mods, dev_mode),
        None => Ok(None),
    }
}

/// Resolves or imports a NaturaLista/iNaturalist taxon into the catalog on demand and returns
/// its complete biological ficha + NaturaLista URL.
pub async fn inspect_naturalista_taxon(
    pool: &PgPool,
    http: &reqwest::Client,
    mods: &ModRegistry,
    endpoints: &TaxonomyEndpoints,
    inaturalist_id: u64,
    user_id: Option<&str>,
    dev_mode: bool,
) -> Result<Option<AlmanacItemDto>, AlmanacError> {
    let taxon_id = TaxonId::INaturalist(inaturalist_id);
    if let Some(existing) = get_almanac_entry(pool, mods, taxon_id, user_id, dev_mode).await?
        && existing.image_url.is_some()
    {
        return Ok(Some(existing));
    }

    let ident = VisionIdentification {
        taxon_id,
        scientific_name: String::new(),
        common_name: None,
        rank: TaxonomicRank::Species,
        subspecies_label: None,
        confidence: 1.0,
    };
    match resolve_species_profile(pool, http, &ident, endpoints).await {
        Ok(_) => get_almanac_entry(pool, mods, taxon_id, user_id, dev_mode).await,
        Err(VisionError::UnknownTaxon(_)) => Ok(None),
        Err(err) => Err(AlmanacError::Vision(err)),
    }
}

#[derive(Debug, Deserialize)]
struct INatSpeciesCountsResponse {
    #[serde(default)]
    results: Vec<INatSpeciesCountRow>,
}

#[derive(Debug, Deserialize)]
struct INatSpeciesCountRow {
    count: i64,
    taxon: INatTaxon,
}

/// Searches NaturaLista (`INATURALIST_TAXA_URL/taxa`) for fauna matching `query`, cross-referencing
/// the player's local collection state.
pub async fn search_naturalista_fauna(
    pool: &PgPool,
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    query: &str,
    user_id: Option<&str>,
    dev_mode: bool,
) -> Result<NaturaListaDiscoveryResponse, AlmanacError> {
    let trimmed = query.trim();
    if trimmed.len() < 2 {
        return Err(AlmanacError::InvalidInput(
            "la búsqueda debe tener al menos 2 caracteres".to_string(),
        ));
    }

    let base = inaturalist_base_url.trim_end_matches('/');
    let url = format!("{base}/taxa");
    let resp = http
        .get(&url)
        .query(&[
            ("q", trimmed),
            ("iconic_taxa", "Animalia"),
            ("rank", "species"),
            ("per_page", "20"),
            ("locale", "es"),
        ])
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !resp.status().is_success() {
        return Err(AlmanacError::Vision(VisionError::Upstream(format!(
            "NaturaLista taxa search returned {}",
            resp.status()
        ))));
    }
    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    let mut entries = parse_naturalista_search_json(&body)?;
    annotate_with_collection_state(pool, &mut entries, user_id, dev_mode).await?;

    Ok(NaturaListaDiscoveryResponse {
        source_url: format!(
            "https://www.naturalista.mx/taxa?q={}",
            trimmed.replace(' ', "+")
        ),
        query_summary: format!(
            "Resultados de fauna en NaturaLista para \"{trimmed}\" ({})",
            entries.len()
        ),
        entries,
    })
}

/// Queries NaturaLista (`INATURALIST_TAXA_URL/observations/species_counts`) for citizen-science
/// wildlife sightings around `coords` (`radius_km` clamped to `1..=100`).
pub async fn nearby_naturalista_fauna(
    pool: &PgPool,
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    coords: GeoPoint,
    radius_km: u32,
    user_id: Option<&str>,
    dev_mode: bool,
) -> Result<NaturaListaDiscoveryResponse, AlmanacError> {
    if !coords.latitude.is_finite()
        || !coords.longitude.is_finite()
        || !(-90.0..=90.0).contains(&coords.latitude)
        || !(-180.0..=180.0).contains(&coords.longitude)
    {
        return Err(AlmanacError::Geo(GeoError::InvalidCoordinates {
            latitude: coords.latitude,
            longitude: coords.longitude,
        }));
    }

    let clamped_radius = radius_km.clamp(1, 100);
    let lat_s = coords.latitude.to_string();
    let lng_s = coords.longitude.to_string();
    let radius_s = clamped_radius.to_string();
    let base = inaturalist_base_url.trim_end_matches('/');
    let url = format!("{base}/observations/species_counts");

    let resp = http
        .get(&url)
        .query(&[
            ("lat", lat_s.as_str()),
            ("lng", lng_s.as_str()),
            ("radius", radius_s.as_str()),
            ("iconic_taxa", "Animalia"),
            ("per_page", "24"),
            ("locale", "es"),
        ])
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !resp.status().is_success() {
        return Err(AlmanacError::Vision(VisionError::Upstream(format!(
            "NaturaLista nearby species returned {}",
            resp.status()
        ))));
    }
    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    let mut entries = parse_naturalista_species_counts_json(&body)?;
    annotate_with_collection_state(pool, &mut entries, user_id, dev_mode).await?;

    Ok(NaturaListaDiscoveryResponse {
        source_url: format!(
            "https://www.naturalista.mx/observations?lat={:.4}&lng={:.4}&radius={clamped_radius}&iconic_taxa=Animalia",
            coords.latitude, coords.longitude
        ),
        query_summary: format!(
            "Fauna observada en NaturaLista a {clamped_radius} km ({} especies)",
            entries.len()
        ),
        entries,
    })
}

/// Parses raw `/v1/taxa?q=...` JSON into [`NaturaListaSpeciesDto`] entries.
pub fn parse_naturalista_search_json(
    raw_json: &str,
) -> Result<Vec<NaturaListaSpeciesDto>, AlmanacError> {
    let parsed: INatTaxaResponse = serde_json::from_str(raw_json)
        .map_err(|err| AlmanacError::Vision(VisionError::Upstream(err.to_string())))?;

    Ok(parsed
        .results
        .into_iter()
        .filter(|taxon| !is_non_fauna_taxon(taxon))
        .map(|taxon| map_inat_taxon_to_dto(&taxon, None))
        .collect())
}

/// Parses raw `/v1/observations/species_counts` JSON into [`NaturaListaSpeciesDto`] entries.
pub fn parse_naturalista_species_counts_json(
    raw_json: &str,
) -> Result<Vec<NaturaListaSpeciesDto>, AlmanacError> {
    let parsed: INatSpeciesCountsResponse = serde_json::from_str(raw_json)
        .map_err(|err| AlmanacError::Vision(VisionError::Upstream(err.to_string())))?;

    Ok(parsed
        .results
        .into_iter()
        .filter(|row| !is_non_fauna_taxon(&row.taxon))
        .map(|row| map_inat_taxon_to_dto(&row.taxon, Some(row.count)))
        .collect())
}

fn map_inat_taxon_to_dto(
    taxon: &INatTaxon,
    local_observations_count: Option<i64>,
) -> NaturaListaSpeciesDto {
    let taxon_id = TaxonId::INaturalist(taxon.id);
    let media: TaxonMediaMeta = extract_inat_media(taxon);
    let iucn_status = taxon
        .conservation_status
        .as_ref()
        .and_then(|cs| cs.status.as_deref())
        .map(parse_iucn)
        .unwrap_or(IucnStatus::Unknown);

    NaturaListaSpeciesDto {
        taxon_id,
        scientific_name: taxon.name.clone(),
        common_name: clean_opt(taxon.preferred_common_name.as_deref()),
        iconic_taxon_name: clean_opt(taxon.iconic_taxon_name.as_deref()),
        iucn_status,
        image_url: media.image_url,
        photo_attribution: media.photo_attribution,
        naturalista_url: naturalista_url_for_taxon(taxon_id),
        wikipedia_url: media.wikipedia_url,
        global_observations_count: media.observations_count,
        local_observations_count,
        discovered: false,
        best_variant: None,
        in_catalog: false,
    }
}

async fn annotate_with_collection_state(
    pool: &PgPool,
    entries: &mut [NaturaListaSpeciesDto],
    user_id: Option<&str>,
    dev_mode: bool,
) -> Result<(), AlmanacError> {
    if entries.is_empty() {
        return Ok(());
    }

    let ids: Vec<i64> = entries
        .iter()
        .filter_map(|e| match e.taxon_id {
            TaxonId::INaturalist(id) => i64::try_from(id).ok(),
            TaxonId::Gbif(_) => None,
        })
        .collect();
    if ids.is_empty() {
        return Ok(());
    }

    let catalog_ids: BTreeSet<i64> = sqlx::query_scalar(
        r#"
        SELECT taxon_external_id
        FROM species_catalog
        WHERE taxon_source = 'i_naturalist' AND taxon_external_id = ANY($1)
        "#,
    )
    .bind(&ids)
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect();

    let cap_rows = sqlx::query(
        r#"
        SELECT
            taxon_external_id,
            BOOL_OR(zone_classification = 'wild') AS has_wild,
            BOOL_OR(zone_classification = 'sanctuary') AS has_sanctuary
        FROM captures
        WHERE taxon_source = 'i_naturalist'
          AND taxon_external_id = ANY($1)
          AND zone_classification IN ('wild', 'sanctuary')
          AND ($2::TEXT IS NULL OR user_id = $2)
        GROUP BY taxon_external_id
        "#,
    )
    .bind(&ids)
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut variant_map: HashMap<i64, String> = HashMap::new();
    for row in cap_rows {
        let ext_id: i64 = row.try_get("taxon_external_id")?;
        let has_wild: bool = row.try_get("has_wild")?;
        let has_sanctuary: bool = row.try_get("has_sanctuary")?;
        if has_wild {
            variant_map.insert(ext_id, "wild".to_string());
        } else if has_sanctuary {
            variant_map.insert(ext_id, "sanctuary".to_string());
        }
    }

    for entry in entries {
        let TaxonId::INaturalist(id_u64) = entry.taxon_id else {
            continue;
        };
        let Ok(id_i64) = i64::try_from(id_u64) else {
            continue;
        };
        entry.in_catalog = catalog_ids.contains(&id_i64);
        if let Some(variant) = variant_map.get(&id_i64) {
            entry.discovered = true;
            entry.best_variant = Some(variant.clone());
        } else if dev_mode {
            entry.discovered = true;
            entry.best_variant = Some("wild".to_string());
        }
    }

    Ok(())
}

fn map_almanac_row(
    row: &sqlx::postgres::PgRow,
    mods: &ModRegistry,
    dev_mode: bool,
) -> Result<Option<AlmanacItemDto>, AlmanacError> {
    let source: String = row.try_get("taxon_source")?;
    let external_id: i64 = row.try_get("taxon_external_id")?;
    let Some(taxon_id) = parse_taxon_parts(&source, external_id) else {
        return Ok(None);
    };

    let diet_raw: String = row.try_get("diet")?;
    let habitat_raw: String = row.try_get("habitat")?;
    let iucn_raw: String = row.try_get("iucn_status")?;
    let defenses_json: serde_json::Value = row.try_get("defenses")?;
    let mass_i64: i64 = row.try_get("average_mass_grams")?;

    let diet = parse_diet(&diet_raw).unwrap_or(Diet::Omnivore);
    let habitat = parse_habitat(&habitat_raw).unwrap_or(Habitat::Terrestrial);
    let iucn_status = parse_iucn(&iucn_raw);
    let defenses: BTreeSet<DefenseTrait> =
        serde_json::from_value(defenses_json).unwrap_or_default();

    let traits = BiologicalTraits {
        taxon_id,
        scientific_name: row.try_get("scientific_name")?,
        common_name: row.try_get("common_name")?,
        phylum: row.try_get("phylum")?,
        taxonomic_class: row.try_get("taxonomic_class")?,
        average_mass_grams: u64::try_from(mass_i64.max(1)).unwrap_or(1),
        diet,
        habitat,
        defenses: defenses.clone(),
        iucn_status,
    };

    let base_card = mods.apply_to_card(generate_card(&traits, CaptureVariant::Wild)?);
    let raw_capture_count: i64 = row.try_get("capture_count")?;
    let has_wild: bool = row.try_get("has_wild")?;
    let has_sanctuary: bool = row.try_get("has_sanctuary")?;
    let real_discovered = raw_capture_count > 0;
    let discovered = real_discovered || dev_mode;
    let capture_count = if dev_mode {
        raw_capture_count.max(1)
    } else {
        raw_capture_count
    };
    let best_variant = if has_wild {
        Some("wild".to_string())
    } else if has_sanctuary {
        Some("sanctuary".to_string())
    } else if dev_mode {
        Some("wild".to_string())
    } else {
        None
    };

    Ok(Some(AlmanacItemDto {
        taxon_id,
        scientific_name: traits.scientific_name,
        common_name: traits.common_name,
        phylum: traits.phylum,
        taxonomic_class: traits.taxonomic_class,
        average_mass_grams: traits.average_mass_grams,
        diet,
        habitat,
        defenses,
        iucn_status,
        category: row.try_get("category")?,
        summary: row.try_get("summary")?,
        region_hint: row.try_get("region_hint")?,
        base_card,
        discovered,
        best_variant,
        capture_count,
        image_url: row.try_get("image_url")?,
        photo_attribution: row.try_get("photo_attribution")?,
        naturalista_url: naturalista_url_for_taxon(taxon_id),
        wikipedia_url: row.try_get("wikipedia_url")?,
        observations_count: row.try_get("naturalista_observations_count")?,
        is_core_almanac: row.try_get("is_core_almanac")?,
        last_captured_at_utc: row.try_get("last_captured_at_utc")?,
        last_zone_name: row.try_get("last_zone_name")?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_naturalista_search_and_species_counts_excluding_plants() {
        let search_json = r#"{
            "results": [
                {
                    "id": 26777,
                    "name": "Ambystoma mexicanum",
                    "preferred_common_name": "Ajolote de Xochimilco",
                    "iconic_taxon_name": "Amphibia",
                    "observations_count": 3064,
                    "conservation_status": { "status": "CR" },
                    "default_photo": {
                        "medium_url": "https://example.org/axolotl.jpg",
                        "attribution": "(c) Ejemplo"
                    },
                    "wikipedia_url": "https://es.wikipedia.org/wiki/Ambystoma_mexicanum"
                },
                {
                    "id": 999999,
                    "name": "Monstera deliciosa",
                    "iconic_taxon_name": "Plantae"
                }
            ]
        }"#;
        let parsed = parse_naturalista_search_json(search_json).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].taxon_id, TaxonId::INaturalist(26_777));
        assert_eq!(
            parsed[0].naturalista_url,
            "https://www.naturalista.mx/taxa/26777"
        );
        assert_eq!(parsed[0].iucn_status, IucnStatus::Cr);
        assert_eq!(
            parsed[0].image_url.as_deref(),
            Some("https://example.org/axolotl.jpg")
        );

        let nearby_json = r#"{
            "results": [
                {
                    "count": 139,
                    "taxon": {
                        "id": 53875,
                        "name": "Ocypode quadrata",
                        "preferred_common_name": "Cangrejo fantasma",
                        "iconic_taxon_name": "Animalia",
                        "observations_count": 48120
                    }
                }
            ]
        }"#;
        let nearby = parse_naturalista_species_counts_json(nearby_json).unwrap();
        assert_eq!(nearby.len(), 1);
        assert_eq!(nearby[0].local_observations_count, Some(139));
        assert_eq!(nearby[0].global_observations_count, Some(48_120));
    }
}
