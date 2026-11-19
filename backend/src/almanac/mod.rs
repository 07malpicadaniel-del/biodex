//! Almanac (encyclopedia dictionary) synchronization and query service.

use std::collections::BTreeSet;

use biodex_core::mods::ModRegistry;
use biodex_core::taxonomy::{
    BiologicalTraits, CaptureVariant, DefenseTrait, Diet, Habitat, IucnStatus, SpeciesCard,
    TaxonId, TaxonomyError, generate_card,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::geo::{GeoError, parse_taxon_parts, taxon_db_parts};
use crate::vision::{parse_diet, parse_habitat, parse_iucn, serialize_slug};

/// Query parameters for `GET /api/v1/almanac`.
#[derive(Debug, Default, Clone, Deserialize)]
pub struct AlmanacFilter {
    /// Case-insensitive substring search against common name, scientific name, or summary.
    pub q: Option<String>,
    /// Exact category filter (`birds`, `mammals`, `reptiles_amphibians`, `insects_arachnids`, `aquatic`).
    pub category: Option<String>,
    /// Exact habitat filter (`terrestrial`, `aerial`, `freshwater`, `marine`, `abyssal`).
    pub habitat: Option<String>,
    /// Optional player identifier used to compute `discovered`, `best_variant`, and `capture_count`.
    pub user_id: Option<String>,
}

/// Complete encyclopedia dictionary entry enriched with procedural card preview and capture badge.
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
    /// `true` when the player has at least one non-discarded capture of this species.
    pub discovered: bool,
    /// Highest-tier variant captured (`"wild"` > `"sanctuary"`), or `None` if undiscovered.
    pub best_variant: Option<String>,
    /// Total non-discarded captures recorded for this species.
    pub capture_count: i64,
}

/// Response returned by `GET /api/v1/almanac`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlmanacListResponse {
    /// Total number of core Almanac species registered in the dictionary.
    pub total_species: usize,
    /// Number of core Almanac species discovered by the queried player (or globally if `user_id` is omitted).
    pub discovered_count: usize,
    /// Filtered list of Almanac entries.
    pub entries: Vec<AlmanacItemDto>,
}

/// Errors produced during Almanac sync or query operations.
#[derive(Debug, Error)]
pub enum AlmanacError {
    /// Invalid taxon ID conversion.
    #[error("geo/taxon error: {0}")]
    Geo(#[from] GeoError),
    /// Invalid biological traits when computing preview card.
    #[error("card generation failed: {0}")]
    Taxonomy(#[from] TaxonomyError),
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

/// Lists core Almanac species with discovery badges and optional search/category/habitat filters.
pub async fn list_almanac(
    pool: &PgPool,
    mods: &ModRegistry,
    filter: &AlmanacFilter,
) -> Result<AlmanacListResponse, AlmanacError> {
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
            COALESCE(cap.capture_count, 0) AS capture_count,
            COALESCE(cap.has_wild, FALSE) AS has_wild,
            COALESCE(cap.has_sanctuary, FALSE) AS has_sanctuary
        FROM species_catalog sc
        LEFT JOIN (
            SELECT
                taxon_source,
                taxon_external_id,
                COUNT(*) AS capture_count,
                BOOL_OR(zone_classification = 'wild') AS has_wild,
                BOOL_OR(zone_classification = 'sanctuary') AS has_sanctuary
            FROM captures
            WHERE zone_classification IN ('wild', 'sanctuary')
              AND ($1::TEXT IS NULL OR user_id = $1)
            GROUP BY taxon_source, taxon_external_id
        ) cap
            ON cap.taxon_source = sc.taxon_source
           AND cap.taxon_external_id = sc.taxon_external_id
        WHERE sc.is_core_almanac = TRUE
        ORDER BY sc.category ASC, COALESCE(sc.common_name, sc.scientific_name) ASC
        "#,
    )
    .bind(filter.user_id.as_deref())
    .fetch_all(pool)
    .await?;

    let mut all_items = Vec::with_capacity(rows.len());
    for row in rows {
        if let Some(item) = map_almanac_row(&row, mods)? {
            all_items.push(item);
        }
    }

    let total_species = all_items.len();
    let discovered_count = all_items.iter().filter(|item| item.discovered).count();

    let query_lower = filter
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_lowercase);
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

    let entries = all_items
        .into_iter()
        .filter(|item| {
            if let Some(cat) = &category_filter
                && item.category.to_lowercase() != *cat
            {
                return false;
            }
            if let Some(hab) = &habitat_filter
                && serialize_slug(&item.habitat) != *hab
            {
                return false;
            }
            if let Some(q) = &query_lower {
                let in_scientific = item.scientific_name.to_lowercase().contains(q);
                let in_common = item
                    .common_name
                    .as_deref()
                    .is_some_and(|name| name.to_lowercase().contains(q));
                let in_summary = item.summary.to_lowercase().contains(q);
                if !in_scientific && !in_common && !in_summary {
                    return false;
                }
            }
            true
        })
        .collect();

    Ok(AlmanacListResponse {
        total_species,
        discovered_count,
        entries,
    })
}

/// Fetches a single Almanac entry by [`TaxonId`] with optional player discovery badge.
pub async fn get_almanac_entry(
    pool: &PgPool,
    mods: &ModRegistry,
    taxon_id: TaxonId,
    user_id: Option<&str>,
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
            COALESCE(cap.capture_count, 0) AS capture_count,
            COALESCE(cap.has_wild, FALSE) AS has_wild,
            COALESCE(cap.has_sanctuary, FALSE) AS has_sanctuary
        FROM species_catalog sc
        LEFT JOIN (
            SELECT
                taxon_source,
                taxon_external_id,
                COUNT(*) AS capture_count,
                BOOL_OR(zone_classification = 'wild') AS has_wild,
                BOOL_OR(zone_classification = 'sanctuary') AS has_sanctuary
            FROM captures
            WHERE zone_classification IN ('wild', 'sanctuary')
              AND ($3::TEXT IS NULL OR user_id = $3)
            GROUP BY taxon_source, taxon_external_id
        ) cap
            ON cap.taxon_source = sc.taxon_source
           AND cap.taxon_external_id = sc.taxon_external_id
        WHERE sc.taxon_source = $1 AND sc.taxon_external_id = $2
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(r) => map_almanac_row(&r, mods),
        None => Ok(None),
    }
}

fn map_almanac_row(
    row: &sqlx::postgres::PgRow,
    mods: &ModRegistry,
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
    let capture_count: i64 = row.try_get("capture_count")?;
    let has_wild: bool = row.try_get("has_wild")?;
    let has_sanctuary: bool = row.try_get("has_sanctuary")?;
    let discovered = capture_count > 0;
    let best_variant = if has_wild {
        Some("wild".to_string())
    } else if has_sanctuary {
        Some("sanctuary".to_string())
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
    }))
}
