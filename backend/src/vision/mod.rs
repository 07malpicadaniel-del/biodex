//! Taxonomic vision classification, `subspecies -> species` rollup, and biological enrichment.

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use biodex_core::crypto::GeoPoint;
use biodex_core::taxonomy::{BiologicalTraits, DefenseTrait, Diet, Habitat, IucnStatus, TaxonId};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::config::TaxonomyEndpoints;
use crate::geo::taxon_db_parts;

const DEFAULT_MOCK_CV_JSON: &str = r#"{
    "results": [
        {
            "combined_score": 94.5,
            "taxon": {
                "id": 130793,
                "name": "Panthera tigris tigris",
                "rank": "subspecies",
                "rank_level": 5,
                "preferred_common_name": "Tigre de Bengala",
                "iconic_taxon_name": "Mammalia",
                "ancestors": [
                    { "id": 40151, "name": "Mammalia", "rank": "class", "rank_level": 50 },
                    { "id": 41944, "name": "Felidae", "rank": "family", "rank_level": 30 },
                    { "id": 41962, "name": "Panthera", "rank": "genus", "rank_level": 20 },
                    { "id": 41967, "name": "Panthera tigris", "rank": "species", "rank_level": 10, "preferred_common_name": "Tigre" }
                ]
            }
        }
    ]
}"#;

/// Normalized taxonomic rank bucket evaluated during vision Species Rollup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaxonomicRank {
    /// Binomial species (`rank_level == 10`).
    Species,
    /// Infraspecific rank (`subspecies`, `variety`, `form`, `rank_level < 10`).
    Subspecies,
    /// Genus (`rank_level == 20`).
    Genus,
    /// Family (`rank_level == 30`).
    Family,
    /// Order, class, phylum, or higher indeterminate rank.
    Higher,
}

impl TaxonomicRank {
    /// Parses iNaturalist `rank` string and optional `rank_level` into [`TaxonomicRank`].
    #[must_use]
    pub fn from_inaturalist(rank: Option<&str>, rank_level: Option<u32>) -> Self {
        match rank.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
            Some("species") => return Self::Species,
            Some("subspecies" | "variety" | "form" | "hybrid" | "infrahybrid") => {
                return Self::Subspecies;
            }
            Some("genus") => return Self::Genus,
            Some("family") => return Self::Family,
            _ => {}
        }
        match rank_level {
            Some(10) => Self::Species,
            Some(level) if level < 10 => Self::Subspecies,
            Some(20) => Self::Genus,
            Some(30) => Self::Family,
            _ => Self::Higher,
        }
    }

    /// Canonical lowercase rank label for diagnostics and errors.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Species => "species",
            Self::Subspecies => "subspecies",
            Self::Genus => "genus",
            Self::Family => "family",
            Self::Higher => "higher",
        }
    }
}

/// Identification result always normalized to atomic `species` rank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionIdentification {
    /// Normalized species-level external taxon ID.
    pub taxon_id: TaxonId,
    /// Binomial scientific name of the base species.
    pub scientific_name: String,
    /// Optional localized vernacular name (Spanish preferred when available).
    pub common_name: Option<String>,
    /// Always [`TaxonomicRank::Species`] after successful rollup.
    pub rank: TaxonomicRank,
    /// Informational infraspecific name when the model initially detected a subspecies/variety/form.
    pub subspecies_label: Option<String>,
    /// Confidence score normalized within `0.0..=1.0`.
    pub confidence: f32,
}

/// Optional development-only taxonomic hint used when `ALLOW_DEV_TAXON_HINT=true`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DevTaxonHint {
    /// Taxon identifier to test against catalog / PostGIS.
    pub taxon_id: TaxonId,
    /// Optional confidence override (`0.0..=1.0`, defaults to `0.95`).
    pub confidence: Option<f32>,
}

/// Errors produced during species vision classification, rank normalization, or trait enrichment.
#[derive(Debug, Error)]
pub enum VisionError {
    /// No vision API token is configured and dev hints / mock mode are disabled.
    #[error("taxonomic vision classifier is not configured (set INATURALIST_API_TOKEN)")]
    ClassifierNotConfigured,
    /// Vision model returned no candidates or confidence fell below threshold.
    #[error("no species detected with sufficient confidence; try a clearer photo")]
    NoSpeciesDetected,
    /// Vision detected flora, fungi, or human instead of wildlife/fauna.
    #[error("only animals can be collected in BioDex (detected '{detected_name}' in {category})")]
    NonFaunaDetected {
        /// Scientific or common name of the non-fauna detection.
        detected_name: String,
        /// Iconic group returned by iNaturalist (e.g., `Plantae`, `Fungi`).
        category: String,
    },
    /// Vision confidence only reached genus, family, or higher taxonomic rank.
    #[error(
        "identification for '{detected_name}' only reached rank '{detected_rank}'; take a closer or sharper photo to reach species level"
    )]
    TaxonomicRankTooBroad {
        /// Scientific name of the broad taxon returned.
        detected_name: String,
        /// Broad rank string (`genus`, `family`, `higher`, etc.).
        detected_rank: String,
    },
    /// Subspecies/variety had no resolvable `species`-rank ancestor.
    #[error("could not resolve base species ancestor for '{detected_name}' ({taxon_id:?})")]
    MissingSpeciesAncestor {
        /// Original detected taxon ID.
        taxon_id: TaxonId,
        /// Original detected infraspecific name.
        detected_name: String,
    },
    /// Upstream taxonomy/vision HTTP error.
    #[error("upstream taxonomy provider failed: {0}")]
    Upstream(String),
    /// Species could not be resolved in local catalog or upstream taxonomy APIs.
    #[error("unknown taxon {0:?}")]
    UnknownTaxon(TaxonId),
    /// Database query error while reading or caching `species_catalog`.
    #[error("species catalog database error: {0}")]
    Database(#[from] sqlx::Error),
}

/// Asynchronous abstraction over image-based taxonomic identification.
pub trait TaxonomyClassifier: Send + Sync {
    /// Identifies the animal species present in raw/JPEG image bytes, normalized to base `species`.
    fn identify<'a>(
        &'a self,
        image_bytes: &'a [u8],
        coords: GeoPoint,
        dev_hint: Option<&'a DevTaxonHint>,
    ) -> Pin<Box<dyn Future<Output = Result<VisionIdentification, VisionError>> + Send + 'a>>;
}

/// Production classifier backed by iNaturalist Computer Vision API + `subspecies -> species` rollup.
#[derive(Debug, Clone)]
pub struct INaturalistVisionClient {
    http: reqwest::Client,
    api_token: Option<String>,
    inaturalist_base_url: String,
    min_confidence: f32,
    allow_dev_taxon_hint: bool,
}

impl INaturalistVisionClient {
    /// Creates a new [`INaturalistVisionClient`].
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        api_token: Option<String>,
        inaturalist_base_url: String,
        min_confidence: f32,
        allow_dev_taxon_hint: bool,
    ) -> Self {
        Self {
            http,
            api_token,
            inaturalist_base_url: inaturalist_base_url.trim_end_matches('/').to_string(),
            min_confidence: min_confidence.clamp(0.0, 1.0),
            allow_dev_taxon_hint,
        }
    }
}

impl TaxonomyClassifier for INaturalistVisionClient {
    fn identify<'a>(
        &'a self,
        image_bytes: &'a [u8],
        coords: GeoPoint,
        dev_hint: Option<&'a DevTaxonHint>,
    ) -> Pin<Box<dyn Future<Output = Result<VisionIdentification, VisionError>> + Send + 'a>> {
        Box::pin(async move {
            if let Some(token) = &self.api_token {
                let image_part = Part::bytes(image_bytes.to_vec())
                    .file_name("capture.jpg")
                    .mime_str("image/jpeg")
                    .map_err(|err| VisionError::Upstream(err.to_string()))?;

                let form = Form::new()
                    .part("image", image_part)
                    .text("lat", coords.latitude.to_string())
                    .text("lng", coords.longitude.to_string())
                    .text("locale", "es");

                let endpoint = format!("{}/computervision/score_image", self.inaturalist_base_url);
                let resp = self
                    .http
                    .post(endpoint)
                    .bearer_auth(token)
                    .multipart(form)
                    .send()
                    .await
                    .map_err(|err| VisionError::Upstream(err.to_string()))?;

                if !resp.status().is_success() {
                    return Err(VisionError::Upstream(format!(
                        "iNaturalist CV returned status {}",
                        resp.status()
                    )));
                }

                let body = resp
                    .text()
                    .await
                    .map_err(|err| VisionError::Upstream(err.to_string()))?;
                return resolve_cv_payload(
                    &self.http,
                    &self.inaturalist_base_url,
                    &body,
                    self.min_confidence,
                )
                .await;
            }

            if self.allow_dev_taxon_hint
                && let Some(hint) = dev_hint
            {
                let confidence = hint.confidence.unwrap_or(0.95).clamp(0.0, 1.0);
                return Ok(VisionIdentification {
                    taxon_id: hint.taxon_id,
                    scientific_name: String::new(),
                    common_name: None,
                    rank: TaxonomicRank::Species,
                    subspecies_label: None,
                    confidence,
                });
            }

            Err(VisionError::ClassifierNotConfigured)
        })
    }
}

/// Offline deterministic classifier (`VISION_PROVIDER=mock`) backed by cached iNaturalist JSON payloads.
#[derive(Debug, Clone)]
pub struct MockVisionClient {
    cv_json: String,
    taxa_json: Option<String>,
    min_confidence: f32,
}

impl MockVisionClient {
    /// Builds a default offline mock client returning a cached subspecies sample that rolls up
    /// to base species `Panthera tigris`.
    #[must_use]
    pub fn default_cached(min_confidence: f32) -> Self {
        Self {
            cv_json: DEFAULT_MOCK_CV_JSON.to_string(),
            taxa_json: None,
            min_confidence: min_confidence.clamp(0.0, 1.0),
        }
    }

    /// Builds an offline mock client with explicit cached CV and optional `/taxa/{id}` JSON responses.
    #[must_use]
    pub fn from_cached_json(
        cv_json: impl Into<String>,
        taxa_json: Option<String>,
        min_confidence: f32,
    ) -> Self {
        Self {
            cv_json: cv_json.into(),
            taxa_json,
            min_confidence: min_confidence.clamp(0.0, 1.0),
        }
    }
}

impl TaxonomyClassifier for MockVisionClient {
    fn identify<'a>(
        &'a self,
        _image_bytes: &'a [u8],
        _coords: GeoPoint,
        _dev_hint: Option<&'a DevTaxonHint>,
    ) -> Pin<Box<dyn Future<Output = Result<VisionIdentification, VisionError>> + Send + 'a>> {
        Box::pin(async move {
            let parsed = parse_inaturalist_cv_candidate(&self.cv_json, self.min_confidence)?;
            match rollup_candidate_from_embedded(&parsed) {
                Ok(ident) => Ok(ident),
                Err(VisionError::MissingSpeciesAncestor { taxon_id, .. }) => {
                    let Some(taxa_raw) = &self.taxa_json else {
                        return Err(VisionError::MissingSpeciesAncestor {
                            taxon_id,
                            detected_name: parsed.taxon.name,
                        });
                    };
                    rollup_candidate_with_taxa_json(&parsed, taxa_raw)
                }
                Err(other) => Err(other),
            }
        })
    }
}

/// Parsed raw iNaturalist CV candidate prior to ancestor HTTP lookup.
#[derive(Debug, Clone, Deserialize)]
pub struct ParsedCvCandidate {
    confidence: f32,
    taxon: INatTaxon,
}

async fn resolve_cv_payload(
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    raw_cv_json: &str,
    min_confidence: f32,
) -> Result<VisionIdentification, VisionError> {
    let candidate = parse_inaturalist_cv_candidate(raw_cv_json, min_confidence)?;
    match rollup_candidate_from_embedded(&candidate) {
        Ok(ident) => Ok(ident),
        Err(VisionError::MissingSpeciesAncestor { taxon_id, .. }) => {
            let id = match taxon_id {
                TaxonId::INaturalist(id) => id,
                TaxonId::Gbif(id) => id,
            };
            let url = format!("{inaturalist_base_url}/taxa/{id}?locale=es");
            let resp = http
                .get(url)
                .send()
                .await
                .map_err(|err| VisionError::Upstream(err.to_string()))?;
            if !resp.status().is_success() {
                return Err(VisionError::Upstream(format!(
                    "iNaturalist taxa lookup for rollup returned {}",
                    resp.status()
                )));
            }
            let body = resp
                .text()
                .await
                .map_err(|err| VisionError::Upstream(err.to_string()))?;
            rollup_candidate_with_taxa_json(&candidate, &body)
        }
        Err(other) => Err(other),
    }
}

/// Builds the canonical NaturaLista Mexico (or GBIF) web URL for a normalized taxon.
#[must_use]
pub fn naturalista_url_for_taxon(taxon_id: TaxonId) -> String {
    match taxon_id {
        TaxonId::INaturalist(id) => format!("https://www.naturalista.mx/taxa/{id}"),
        TaxonId::Gbif(id) => format!("https://www.gbif.org/species/{id}"),
    }
}

/// Parses the top fauna candidate from an iNaturalist CV result and validates confidence (`0..100%`).
pub fn parse_inaturalist_cv_candidate(
    raw_json: &str,
    min_confidence: f32,
) -> Result<ParsedCvCandidate, VisionError> {
    let parsed: INatCvResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    if parsed.results.is_empty() {
        return Err(VisionError::NoSpeciesDetected);
    }

    let mut top_non_fauna: Option<(String, String)> = None;
    for result in parsed.results {
        let raw_score = result.combined_score.or(result.vision_score).unwrap_or(0.0);
        // iNaturalist Computer Vision `score_image` always reports percentage scores in `0.0..=100.0`.
        let confidence = (raw_score / 100.0).clamp(0.0, 1.0);
        if confidence < min_confidence {
            continue;
        }
        if is_non_fauna_taxon(&result.taxon) {
            if top_non_fauna.is_none() {
                let category = result
                    .taxon
                    .iconic_taxon_name
                    .clone()
                    .unwrap_or_else(|| "NonAnimalia".to_string());
                top_non_fauna = Some((result.taxon.name.clone(), category));
            }
            continue;
        }

        return Ok(ParsedCvCandidate {
            confidence,
            taxon: result.taxon,
        });
    }

    if let Some((detected_name, category)) = top_non_fauna {
        return Err(VisionError::NonFaunaDetected {
            detected_name,
            category,
        });
    }

    Err(VisionError::NoSpeciesDetected)
}

pub(crate) fn is_non_fauna_taxon(taxon: &INatTaxon) -> bool {
    if taxon.id == 43_584 || taxon.name.trim().eq_ignore_ascii_case("Homo sapiens") {
        return true;
    }
    matches!(
        taxon
            .iconic_taxon_name
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("plantae" | "fungi" | "chromista" | "protozoa" | "bacteria" | "viruses")
    )
}

/// Normalizes a candidate using embedded `ancestors` if available; rejects broad ranks.
pub fn rollup_candidate_from_embedded(
    candidate: &ParsedCvCandidate,
) -> Result<VisionIdentification, VisionError> {
    let rank = TaxonomicRank::from_inaturalist(
        candidate.taxon.rank.as_deref(),
        candidate.taxon.rank_level,
    );
    match rank {
        TaxonomicRank::Species => Ok(VisionIdentification {
            taxon_id: TaxonId::INaturalist(candidate.taxon.id),
            scientific_name: candidate.taxon.name.trim().to_string(),
            common_name: clean_opt(candidate.taxon.preferred_common_name.as_deref()),
            rank: TaxonomicRank::Species,
            subspecies_label: None,
            confidence: candidate.confidence,
        }),
        TaxonomicRank::Subspecies => {
            if let Some(base) = find_species_ancestor(&candidate.taxon.ancestors) {
                Ok(VisionIdentification {
                    taxon_id: TaxonId::INaturalist(base.id),
                    scientific_name: base.name.trim().to_string(),
                    common_name: clean_opt(base.preferred_common_name.as_deref()),
                    rank: TaxonomicRank::Species,
                    subspecies_label: Some(candidate.taxon.name.trim().to_string()),
                    confidence: candidate.confidence,
                })
            } else {
                Err(VisionError::MissingSpeciesAncestor {
                    taxon_id: TaxonId::INaturalist(candidate.taxon.id),
                    detected_name: candidate.taxon.name.clone(),
                })
            }
        }
        TaxonomicRank::Genus | TaxonomicRank::Family | TaxonomicRank::Higher => {
            let detected_rank = candidate
                .taxon
                .rank
                .clone()
                .unwrap_or_else(|| rank.as_str().to_string());
            Err(VisionError::TaxonomicRankTooBroad {
                detected_name: candidate.taxon.name.clone(),
                detected_rank,
            })
        }
    }
}

/// Resolves a subspecies candidate using a fetched `/v1/taxa/{id}?locale=es` JSON payload.
pub fn rollup_candidate_with_taxa_json(
    candidate: &ParsedCvCandidate,
    taxa_json: &str,
) -> Result<VisionIdentification, VisionError> {
    let parsed: INatTaxaResponse =
        serde_json::from_str(taxa_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    let taxon = parsed
        .results
        .into_iter()
        .next()
        .ok_or(VisionError::UnknownTaxon(TaxonId::INaturalist(
            candidate.taxon.id,
        )))?;

    let base = find_species_ancestor(&taxon.ancestors).ok_or_else(|| {
        VisionError::MissingSpeciesAncestor {
            taxon_id: TaxonId::INaturalist(candidate.taxon.id),
            detected_name: candidate.taxon.name.clone(),
        }
    })?;

    Ok(VisionIdentification {
        taxon_id: TaxonId::INaturalist(base.id),
        scientific_name: base.name.trim().to_string(),
        common_name: clean_opt(base.preferred_common_name.as_deref()),
        rank: TaxonomicRank::Species,
        subspecies_label: Some(candidate.taxon.name.trim().to_string()),
        confidence: candidate.confidence,
    })
}

fn find_species_ancestor(ancestors: &[INatAncestor]) -> Option<&INatAncestor> {
    ancestors.iter().rev().find(|ancestor| {
        TaxonomicRank::from_inaturalist(ancestor.rank.as_deref(), ancestor.rank_level)
            == TaxonomicRank::Species
    })
}

/// Public photo and encyclopedia metadata fetched from NaturaLista / iNaturalist.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TaxonMediaMeta {
    /// Medium or square wildlife reference photograph URL.
    pub image_url: Option<String>,
    /// Photographer / license attribution string.
    pub photo_attribution: Option<String>,
    /// Spanish or canonical Wikipedia encyclopedia URL.
    pub wikipedia_url: Option<String>,
    /// Total verifiable community observations logged in NaturaLista / iNaturalist.
    pub observations_count: Option<i64>,
}

/// Complete resolved species profile including traits and optional NaturaLista media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpeciesProfile {
    /// Normalized biological traits used for catalog and procedural generation.
    pub traits: BiologicalTraits,
    /// Optional photograph & NaturaLista metadata.
    pub media: TaxonMediaMeta,
}

/// Resolves full [`BiologicalTraits`] from `species_catalog` or enriches via iNaturalist + GBIF + Wikidata.
pub async fn resolve_biological_traits(
    pool: &PgPool,
    http: &reqwest::Client,
    ident: &VisionIdentification,
    endpoints: &TaxonomyEndpoints,
) -> Result<BiologicalTraits, VisionError> {
    resolve_species_profile(pool, http, ident, endpoints)
        .await
        .map(|profile| profile.traits)
}

/// Resolves [`BiologicalTraits`] and caches NaturaLista photograph/Wikipedia metadata.
pub async fn resolve_species_profile(
    pool: &PgPool,
    http: &reqwest::Client,
    ident: &VisionIdentification,
    endpoints: &TaxonomyEndpoints,
) -> Result<ResolvedSpeciesProfile, VisionError> {
    if let Some(cached) = fetch_cached_profile(pool, ident.taxon_id, &ident.scientific_name).await?
    {
        let mut media = cached.media;
        if media.image_url.is_none()
            && let TaxonId::INaturalist(id) = ident.taxon_id
            && let Ok(fresh) =
                fetch_inaturalist_taxon_with_media(http, &endpoints.inaturalist_base_url, id).await
        {
            media = fresh.1;
            let _ = backfill_taxon_media(pool, ident.taxon_id, &media).await;
        }
        return Ok(ResolvedSpeciesProfile {
            traits: cached.traits,
            media,
        });
    }

    let (mut traits, mut summary, media) = match ident.taxon_id {
        TaxonId::INaturalist(id) => {
            let (mut base, media) =
                fetch_inaturalist_taxon_with_media(http, &endpoints.inaturalist_base_url, id)
                    .await?;
            if base.common_name.is_none() {
                base.common_name = ident.common_name.clone();
            }
            (base, String::new(), media)
        }
        TaxonId::Gbif(key) => {
            let base = fetch_gbif_taxon(http, &endpoints.gbif_base_url, key).await?;
            (base, String::new(), TaxonMediaMeta::default())
        }
    };

    // Enrich taxonomy hierarchy & IUCN status from GBIF (graceful degradation on network failure).
    let mut order_hint: Option<String> = None;
    let mut family_hint: Option<String> = None;
    if let Ok(Some(gbif)) =
        fetch_gbif_match(http, &endpoints.gbif_base_url, &traits.scientific_name).await
    {
        if let Some(phylum) = gbif.phylum.filter(|s| !s.trim().is_empty()) {
            traits.phylum = phylum;
        }
        if let Some(class_name) = gbif.class.filter(|s| !s.trim().is_empty()) {
            traits.taxonomic_class = class_name;
        }
        order_hint = gbif.order;
        family_hint = gbif.family;

        if traits.iucn_status == IucnStatus::Unknown
            && let Some(species_key) = gbif.species_key.or(gbif.usage_key)
            && let Ok(Some(iucn)) =
                fetch_gbif_iucn(http, &endpoints.gbif_base_url, species_key).await
        {
            traits.iucn_status = iucn;
        }
    }

    // Re-evaluate biological defaults using full (phylum, class, order, family).
    let inferred = infer_defaults_from_taxonomy(
        &traits.phylum,
        &traits.taxonomic_class,
        order_hint.as_deref(),
        family_hint.as_deref(),
    );
    traits.diet = inferred.diet;
    traits.habitat = inferred.habitat;
    traits.average_mass_grams = inferred.average_mass_grams;
    traits.defenses.extend(inferred.defenses);

    // Enrich adult mass, IUCN fallback, and Spanish summary from Wikidata when available.
    if let Ok(Some(wd)) =
        fetch_wikidata_facts(http, &endpoints.wikidata_api_url, &traits.scientific_name).await
    {
        if let Some(mass_g) = wd.mass_grams {
            traits.average_mass_grams = mass_g.max(1);
        }
        if traits.iucn_status == IucnStatus::Unknown
            && let Some(iucn) = wd.iucn_status
        {
            traits.iucn_status = iucn;
        }
        if traits.common_name.is_none() {
            traits.common_name = wd.label_es;
        }
        if let Some(desc) = wd.description_es {
            summary = desc;
        }
    }

    if summary.trim().is_empty() {
        summary = format!(
            "Especie {} perteneciente a {} ({}).",
            traits.scientific_name, traits.taxonomic_class, traits.phylum
        );
    }

    let category = infer_almanac_category(&traits.taxonomic_class, traits.habitat);
    cache_traits(pool, &traits, &summary, category, &media).await?;
    Ok(ResolvedSpeciesProfile { traits, media })
}

async fn fetch_cached_profile(
    pool: &PgPool,
    taxon_id: TaxonId,
    scientific_name: &str,
) -> Result<Option<ResolvedSpeciesProfile>, VisionError> {
    let (taxon_source, taxon_external_id) =
        taxon_db_parts(taxon_id).map_err(|_| VisionError::UnknownTaxon(taxon_id))?;

    let row = sqlx::query(
        r#"
        SELECT taxon_source, taxon_external_id, scientific_name, common_name, phylum, taxonomic_class,
               average_mass_grams, diet, habitat, defenses, iucn_status,
               image_url, photo_attribution, wikipedia_url, naturalista_observations_count
        FROM species_catalog
        WHERE (taxon_source = $1 AND taxon_external_id = $2)
           OR ($3 <> '' AND LOWER(scientific_name) = LOWER($3))
        ORDER BY (taxon_source = $1 AND taxon_external_id = $2) DESC
        LIMIT 1
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(scientific_name.trim())
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
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

    Ok(Some(ResolvedSpeciesProfile {
        traits: BiologicalTraits {
            taxon_id,
            scientific_name: row.try_get("scientific_name")?,
            common_name: row.try_get("common_name")?,
            phylum: row.try_get("phylum")?,
            taxonomic_class: row.try_get("taxonomic_class")?,
            average_mass_grams: u64::try_from(mass_i64.max(1)).unwrap_or(1),
            diet,
            habitat,
            defenses,
            iucn_status,
        },
        media: TaxonMediaMeta {
            image_url: row.try_get("image_url")?,
            photo_attribution: row.try_get("photo_attribution")?,
            wikipedia_url: row.try_get("wikipedia_url")?,
            observations_count: row.try_get("naturalista_observations_count")?,
        },
    }))
}

async fn backfill_taxon_media(
    pool: &PgPool,
    taxon_id: TaxonId,
    media: &TaxonMediaMeta,
) -> Result<(), VisionError> {
    let (taxon_source, taxon_external_id) =
        taxon_db_parts(taxon_id).map_err(|_| VisionError::UnknownTaxon(taxon_id))?;

    sqlx::query(
        r#"
        UPDATE species_catalog
        SET image_url = COALESCE(image_url, $3),
            photo_attribution = COALESCE(photo_attribution, $4),
            wikipedia_url = COALESCE(wikipedia_url, $5),
            naturalista_observations_count = COALESCE($6, naturalista_observations_count)
        WHERE taxon_source = $1 AND taxon_external_id = $2
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(&media.image_url)
    .bind(&media.photo_attribution)
    .bind(&media.wikipedia_url)
    .bind(media.observations_count)
    .execute(pool)
    .await?;

    Ok(())
}

async fn cache_traits(
    pool: &PgPool,
    traits: &BiologicalTraits,
    summary: &str,
    category: &str,
    media: &TaxonMediaMeta,
) -> Result<(), VisionError> {
    let (taxon_source, taxon_external_id) =
        taxon_db_parts(traits.taxon_id).map_err(|_| VisionError::UnknownTaxon(traits.taxon_id))?;

    let diet = serialize_slug(&traits.diet);
    let habitat = serialize_slug(&traits.habitat);
    let iucn = serialize_slug(&traits.iucn_status);
    let defenses =
        serde_json::to_value(&traits.defenses).unwrap_or(serde_json::Value::Array(vec![]));
    let mass = i64::try_from(traits.average_mass_grams).unwrap_or(i64::MAX);

    sqlx::query(
        r#"
        INSERT INTO species_catalog (
            taxon_source, taxon_external_id, scientific_name, common_name,
            phylum, taxonomic_class, average_mass_grams, diet, habitat, defenses,
            iucn_status, summary, category, is_core_almanac,
            image_url, photo_attribution, wikipedia_url, naturalista_observations_count
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, FALSE, $14, $15, $16, $17)
        ON CONFLICT (taxon_source, taxon_external_id) DO UPDATE SET
            image_url = COALESCE(species_catalog.image_url, EXCLUDED.image_url),
            photo_attribution = COALESCE(species_catalog.photo_attribution, EXCLUDED.photo_attribution),
            wikipedia_url = COALESCE(species_catalog.wikipedia_url, EXCLUDED.wikipedia_url),
            naturalista_observations_count = COALESCE(EXCLUDED.naturalista_observations_count, species_catalog.naturalista_observations_count)
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(&traits.scientific_name)
    .bind(&traits.common_name)
    .bind(&traits.phylum)
    .bind(&traits.taxonomic_class)
    .bind(mass)
    .bind(diet)
    .bind(habitat)
    .bind(defenses)
    .bind(iucn)
    .bind(summary)
    .bind(category)
    .bind(&media.image_url)
    .bind(&media.photo_attribution)
    .bind(&media.wikipedia_url)
    .bind(media.observations_count)
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_inaturalist_taxon_with_media(
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    taxon_id: u64,
) -> Result<(BiologicalTraits, TaxonMediaMeta), VisionError> {
    let url = format!("{inaturalist_base_url}/taxa/{taxon_id}?locale=es");
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if resp.status().as_u16() == 404 {
        return Err(VisionError::UnknownTaxon(TaxonId::INaturalist(taxon_id)));
    }
    if !resp.status().is_success() {
        return Err(VisionError::Upstream(format!(
            "iNaturalist taxa lookup returned {}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    parse_inaturalist_taxon_response_with_media(&body, TaxonId::INaturalist(taxon_id))
}

async fn fetch_gbif_taxon(
    http: &reqwest::Client,
    gbif_base_url: &str,
    taxon_key: u64,
) -> Result<BiologicalTraits, VisionError> {
    let url = format!("{gbif_base_url}/species/{taxon_key}");
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if resp.status().as_u16() == 404 {
        return Err(VisionError::UnknownTaxon(TaxonId::Gbif(taxon_key)));
    }
    if !resp.status().is_success() {
        return Err(VisionError::Upstream(format!(
            "GBIF species lookup returned {}",
            resp.status()
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    parse_gbif_taxon_response(&body, TaxonId::Gbif(taxon_key))
}

async fn fetch_gbif_match(
    http: &reqwest::Client,
    gbif_base_url: &str,
    scientific_name: &str,
) -> Result<Option<GbifMatchResponse>, VisionError> {
    let url = format!("{gbif_base_url}/species/match");
    let resp = http
        .get(url)
        .query(&[("name", scientific_name)])
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !resp.status().is_success() {
        return Ok(None);
    }
    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    parse_gbif_match_response(&body)
}

async fn fetch_gbif_iucn(
    http: &reqwest::Client,
    gbif_base_url: &str,
    species_key: u64,
) -> Result<Option<IucnStatus>, VisionError> {
    let url = format!("{gbif_base_url}/species/{species_key}/iucnRedListCategory");
    let resp = http
        .get(url)
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !resp.status().is_success() {
        return Ok(None);
    }
    let body = resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    Ok(parse_gbif_iucn_response(&body))
}

async fn fetch_wikidata_facts(
    http: &reqwest::Client,
    wikidata_api_url: &str,
    scientific_name: &str,
) -> Result<Option<WikidataTaxonFacts>, VisionError> {
    let search_resp = http
        .get(wikidata_api_url)
        .query(&[
            ("action", "wbsearchentities"),
            ("search", scientific_name),
            ("language", "es"),
            ("format", "json"),
            ("limit", "3"),
        ])
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !search_resp.status().is_success() {
        return Ok(None);
    }
    let search_body = search_resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    let search: WikidataSearchResponse = match serde_json::from_str(&search_body) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(None),
    };

    let qids: Vec<String> = search.search.into_iter().map(|entry| entry.id).collect();
    if qids.is_empty() {
        return Ok(None);
    }

    let joined_qids = qids.join("|");
    let entities_resp = http
        .get(wikidata_api_url)
        .query(&[
            ("action", "wbgetentities"),
            ("ids", joined_qids.as_str()),
            ("props", "claims|labels|descriptions"),
            ("languages", "es|en"),
            ("format", "json"),
        ])
        .send()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;

    if !entities_resp.status().is_success() {
        return Ok(None);
    }
    let entities_body = entities_resp
        .text()
        .await
        .map_err(|err| VisionError::Upstream(err.to_string()))?;
    Ok(parse_wikidata_entities_response(
        &entities_body,
        scientific_name,
    ))
}

#[derive(Debug, Deserialize)]
struct INatCvResponse {
    #[serde(default)]
    results: Vec<INatCvResult>,
}

#[derive(Debug, Deserialize)]
struct INatCvResult {
    combined_score: Option<f32>,
    vision_score: Option<f32>,
    taxon: INatTaxon,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct INatTaxaResponse {
    #[serde(default)]
    pub(crate) results: Vec<INatTaxon>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct INatDefaultPhoto {
    pub(crate) square_url: Option<String>,
    pub(crate) medium_url: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) attribution: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct INatTaxon {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) rank: Option<String>,
    pub(crate) rank_level: Option<u32>,
    pub(crate) preferred_common_name: Option<String>,
    pub(crate) iconic_taxon_name: Option<String>,
    pub(crate) conservation_status: Option<INatConservationStatus>,
    pub(crate) default_photo: Option<INatDefaultPhoto>,
    pub(crate) wikipedia_url: Option<String>,
    pub(crate) observations_count: Option<i64>,
    #[serde(default)]
    pub(crate) ancestors: Vec<INatAncestor>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct INatAncestor {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) rank: Option<String>,
    pub(crate) rank_level: Option<u32>,
    pub(crate) preferred_common_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct INatConservationStatus {
    pub(crate) status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GbifSpeciesResponse {
    #[serde(rename = "canonicalName")]
    canonical_name: Option<String>,
    #[serde(rename = "scientificName")]
    scientific_name: Option<String>,
    #[serde(rename = "vernacularName")]
    vernacular_name: Option<String>,
    phylum: Option<String>,
    class: Option<String>,
}

/// Subset of GBIF `/v1/species/match` used for biological enrichment.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct GbifMatchResponse {
    /// Usage key returned by GBIF.
    #[serde(rename = "usageKey")]
    pub usage_key: Option<u64>,
    /// Base species key in GBIF backbone.
    #[serde(rename = "speciesKey")]
    pub species_key: Option<u64>,
    /// Canonical binomial name.
    #[serde(rename = "canonicalName")]
    pub canonical_name: Option<String>,
    /// Phylum name.
    pub phylum: Option<String>,
    /// Taxonomic class name.
    pub class: Option<String>,
    /// Taxonomic order name.
    pub order: Option<String>,
    /// Taxonomic family name.
    pub family: Option<String>,
    /// Match type (`EXACT`, `FUZZY`, `HIGHERRANK`, `NONE`).
    #[serde(rename = "matchType")]
    pub match_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GbifIucnResponse {
    code: Option<String>,
    category: Option<String>,
}

/// Extracted factual traits from a matched Wikidata entity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikidataTaxonFacts {
    /// Adult body mass in integer grams (derived from `P2067`).
    pub mass_grams: Option<u64>,
    /// IUCN status fallback (derived from `P141`).
    pub iucn_status: Option<IucnStatus>,
    /// Spanish common label (`labels.es`).
    pub label_es: Option<String>,
    /// Spanish short summary (`descriptions.es`).
    pub description_es: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikidataSearchResponse {
    #[serde(default)]
    search: Vec<WikidataSearchItem>,
}

#[derive(Debug, Deserialize)]
struct WikidataSearchItem {
    id: String,
}

#[derive(Debug, Deserialize)]
struct WikidataEntitiesResponse {
    #[serde(default)]
    entities: serde_json::Map<String, serde_json::Value>,
}

/// Parses an iNaturalist `/v1/taxa/{id}` JSON response into default [`BiologicalTraits`].
pub fn parse_inaturalist_taxon_response(
    raw_json: &str,
    taxon_id: TaxonId,
) -> Result<BiologicalTraits, VisionError> {
    parse_inaturalist_taxon_response_with_media(raw_json, taxon_id).map(|(traits, _)| traits)
}

pub(crate) fn extract_inat_media(taxon: &INatTaxon) -> TaxonMediaMeta {
    let image_url = taxon.default_photo.as_ref().and_then(|p| {
        clean_opt(p.medium_url.as_deref())
            .or_else(|| clean_opt(p.square_url.as_deref()))
            .or_else(|| clean_opt(p.url.as_deref()))
    });
    let photo_attribution = taxon
        .default_photo
        .as_ref()
        .and_then(|p| clean_opt(p.attribution.as_deref()));

    TaxonMediaMeta {
        image_url,
        photo_attribution,
        wikipedia_url: clean_opt(taxon.wikipedia_url.as_deref()),
        observations_count: taxon.observations_count.filter(|&c| c >= 0),
    }
}

/// Parses an iNaturalist `/v1/taxa/{id}` JSON payload into [`BiologicalTraits`] + [`TaxonMediaMeta`].
pub fn parse_inaturalist_taxon_response_with_media(
    raw_json: &str,
    taxon_id: TaxonId,
) -> Result<(BiologicalTraits, TaxonMediaMeta), VisionError> {
    let parsed: INatTaxaResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    let taxon = parsed
        .results
        .into_iter()
        .next()
        .ok_or(VisionError::UnknownTaxon(taxon_id))?;

    let media = extract_inat_media(&taxon);
    let iconic = taxon.iconic_taxon_name.as_deref().unwrap_or("Animalia");
    let inferred = infer_defaults_from_taxonomy(iconic, iconic, None, None);
    let iucn_status = taxon
        .conservation_status
        .and_then(|cs| cs.status)
        .map(|code| parse_iucn(&code))
        .unwrap_or(IucnStatus::Unknown);

    let traits = BiologicalTraits {
        taxon_id,
        scientific_name: taxon.name,
        common_name: clean_opt(taxon.preferred_common_name.as_deref()),
        phylum: inferred.phylum.to_string(),
        taxonomic_class: inferred.taxonomic_class.to_string(),
        average_mass_grams: inferred.average_mass_grams,
        diet: inferred.diet,
        habitat: inferred.habitat,
        defenses: inferred.defenses,
        iucn_status,
    };
    Ok((traits, media))
}

/// Parses a GBIF `/v1/species/{key}` JSON response into default [`BiologicalTraits`].
pub fn parse_gbif_taxon_response(
    raw_json: &str,
    taxon_id: TaxonId,
) -> Result<BiologicalTraits, VisionError> {
    let parsed: GbifSpeciesResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    let scientific_name = parsed
        .canonical_name
        .or(parsed.scientific_name)
        .filter(|name| !name.trim().is_empty())
        .ok_or(VisionError::UnknownTaxon(taxon_id))?;

    let taxonomic_class = parsed.class.unwrap_or_else(|| "Animalia".to_string());
    let phylum = parsed.phylum.unwrap_or_else(|| "Animalia".to_string());
    let inferred = infer_defaults_from_taxonomy(&phylum, &taxonomic_class, None, None);

    Ok(BiologicalTraits {
        taxon_id,
        scientific_name,
        common_name: clean_opt(parsed.vernacular_name.as_deref()),
        phylum,
        taxonomic_class,
        average_mass_grams: inferred.average_mass_grams,
        diet: inferred.diet,
        habitat: inferred.habitat,
        defenses: inferred.defenses,
        iucn_status: IucnStatus::Unknown,
    })
}

/// Parses a GBIF `/v1/species/match` JSON response.
pub fn parse_gbif_match_response(raw_json: &str) -> Result<Option<GbifMatchResponse>, VisionError> {
    let parsed: GbifMatchResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    if parsed.match_type.as_deref() == Some("NONE") || parsed.usage_key.is_none() {
        return Ok(None);
    }
    Ok(Some(parsed))
}

/// Parses a GBIF `/v1/species/{key}/iucnRedListCategory` JSON response.
#[must_use]
pub fn parse_gbif_iucn_response(raw_json: &str) -> Option<IucnStatus> {
    let parsed: GbifIucnResponse = serde_json::from_str(raw_json).ok()?;
    let raw = parsed.code.or(parsed.category)?;
    let status = parse_iucn(&raw);
    if status == IucnStatus::Unknown {
        None
    } else {
        Some(status)
    }
}

/// Extracts Wikidata mass (`P2067`), IUCN (`P141`), and Spanish label/description for an entity
/// whose `P225` taxon name matches `scientific_name`.
#[must_use]
pub fn parse_wikidata_entities_response(
    raw_json: &str,
    scientific_name: &str,
) -> Option<WikidataTaxonFacts> {
    let parsed: WikidataEntitiesResponse = serde_json::from_str(raw_json).ok()?;
    let target_lower = scientific_name.trim().to_ascii_lowercase();

    for entity in parsed.entities.values() {
        let claims = entity.get("claims")?;
        if !wikidata_taxon_matches(claims, &target_lower) {
            continue;
        }

        let mass_grams = extract_wikidata_adult_mass_grams(claims);
        let iucn_status = extract_wikidata_iucn(claims);
        let label_es = entity
            .pointer("/labels/es/value")
            .and_then(|v| v.as_str())
            .and_then(|s| clean_opt(Some(s)));
        let description_es = entity
            .pointer("/descriptions/es/value")
            .and_then(|v| v.as_str())
            .and_then(|s| clean_opt(Some(s)));

        return Some(WikidataTaxonFacts {
            mass_grams,
            iucn_status,
            label_es,
            description_es,
        });
    }

    None
}

fn wikidata_taxon_matches(claims: &serde_json::Value, target_lower: &str) -> bool {
    let Some(p225_list) = claims.get("P225").and_then(|v| v.as_array()) else {
        return false;
    };
    p225_list.iter().any(|claim| {
        claim
            .pointer("/mainsnak/datavalue/value")
            .and_then(|v| v.as_str())
            .is_some_and(|val| val.trim().to_ascii_lowercase() == target_lower)
    })
}

fn extract_wikidata_adult_mass_grams(claims: &serde_json::Value) -> Option<u64> {
    let list = claims.get("P2067")?.as_array()?;
    let mut gram_samples = Vec::new();

    for claim in list {
        let Some(amount_str) = claim
            .pointer("/mainsnak/datavalue/value/amount")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        let Ok(amount) = amount_str.trim().parse::<f64>() else {
            continue;
        };
        if !amount.is_finite() || amount <= 0.0 {
            continue;
        }

        let unit_url = claim
            .pointer("/mainsnak/datavalue/value/unit")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let grams_f64 = if unit_url.ends_with("Q11570") {
            amount * 1_000.0 // kilogram
        } else if unit_url.ends_with("Q41803") {
            amount // gram
        } else if unit_url.ends_with("Q191118") {
            amount * 1_000_000.0 // metric ton
        } else if unit_url.ends_with("Q48013") {
            amount / 1_000.0 // milligram
        } else {
            continue;
        };

        if (1.0..=250_000_000.0).contains(&grams_f64) {
            gram_samples.push(grams_f64.round() as u64);
        }
    }

    let max_grams = gram_samples.iter().copied().max()?;
    // Filter out neonatal / birth weights (e.g., 1.19 kg vs 180 kg adult tiger).
    let adult_threshold = max_grams / 5;
    let adult_samples: Vec<u64> = gram_samples
        .into_iter()
        .filter(|g| *g >= adult_threshold)
        .collect();
    let sum: u128 = adult_samples.iter().map(|g| u128::from(*g)).sum();
    let avg = sum / u128::try_from(adult_samples.len()).ok()?;
    u64::try_from(avg).ok()
}

fn extract_wikidata_iucn(claims: &serde_json::Value) -> Option<IucnStatus> {
    let list = claims.get("P141")?.as_array()?;
    for claim in list {
        let Some(qid) = claim
            .pointer("/mainsnak/datavalue/value/id")
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        let status = match qid {
            "Q211005" => IucnStatus::Lc,
            "Q719675" => IucnStatus::Nt,
            "Q278113" => IucnStatus::Vu,
            "Q11394" => IucnStatus::En,
            "Q219127" => IucnStatus::Cr,
            _ => continue,
        };
        return Some(status);
    }
    None
}

struct InferredTaxonomyDefaults {
    phylum: &'static str,
    taxonomic_class: &'static str,
    habitat: Habitat,
    diet: Diet,
    average_mass_grams: u64,
    defenses: BTreeSet<DefenseTrait>,
}

fn infer_defaults_from_taxonomy(
    phylum_or_iconic: &str,
    class_name: &str,
    order: Option<&str>,
    family: Option<&str>,
) -> InferredTaxonomyDefaults {
    let mut defenses = BTreeSet::new();

    let (phylum, taxonomic_class, mut habitat, mut diet, mut mass_g) = match class_name {
        "Aves" => ("Chordata", "Aves", Habitat::Aerial, Diet::Omnivore, 250),
        "Mammalia" => (
            "Chordata",
            "Mammalia",
            Habitat::Terrestrial,
            Diet::Omnivore,
            5_000,
        ),
        "Reptilia" => (
            "Chordata",
            "Reptilia",
            Habitat::Terrestrial,
            Diet::Carnivore,
            1_200,
        ),
        "Amphibia" => (
            "Chordata",
            "Amphibia",
            Habitat::Freshwater,
            Diet::Carnivore,
            40,
        ),
        "Actinopterygii" | "Chondrichthyes" => (
            "Chordata",
            "Actinopterygii",
            Habitat::Marine,
            Diet::Carnivore,
            1_500,
        ),
        "Insecta" => (
            "Arthropoda",
            "Insecta",
            Habitat::Terrestrial,
            Diet::Herbivore,
            2,
        ),
        "Arachnida" => (
            "Arthropoda",
            "Arachnida",
            Habitat::Terrestrial,
            Diet::Carnivore,
            5,
        ),
        "Mollusca" | "Cephalopoda" => (
            "Mollusca",
            "Cephalopoda",
            Habitat::Marine,
            Diet::Carnivore,
            800,
        ),
        _ => match phylum_or_iconic {
            "Arthropoda" => (
                "Arthropoda",
                "Insecta",
                Habitat::Terrestrial,
                Diet::Herbivore,
                3,
            ),
            "Mollusca" => (
                "Mollusca",
                "Gastropoda",
                Habitat::Marine,
                Diet::Herbivore,
                150,
            ),
            _ => (
                "Animalia",
                "Unknown",
                Habitat::Terrestrial,
                Diet::Omnivore,
                500,
            ),
        },
    };

    match order.unwrap_or("") {
        "Carnivora" => {
            diet = Diet::Carnivore;
            mass_g = 15_000;
        }
        "Artiodactyla" | "Perissodactyla" => {
            diet = Diet::Herbivore;
            mass_g = 120_000;
        }
        "Cetacea" | "Sirenia" => {
            habitat = Habitat::Marine;
            diet = if order == Some("Sirenia") {
                Diet::Herbivore
            } else {
                Diet::Carnivore
            };
            mass_g = 400_000;
        }
        "Chiroptera" => {
            habitat = Habitat::Aerial;
            mass_g = 35;
        }
        "Testudines" => {
            defenses.insert(DefenseTrait::ShellArmor);
        }
        "Lepidoptera" => {
            habitat = Habitat::Aerial;
            diet = Diet::Herbivore;
            mass_g = 1;
        }
        "Odonata" => {
            habitat = Habitat::Aerial;
            diet = Diet::Carnivore;
            defenses.insert(DefenseTrait::Speed);
            mass_g = 2;
        }
        "Anseriformes" | "Pelecaniformes" | "Charadriiformes" => {
            habitat = Habitat::Marine;
        }
        _ => {}
    }

    match family.unwrap_or("") {
        "Felidae" | "Accipitridae" | "Falconidae" => {
            diet = Diet::Carnivore;
            defenses.insert(DefenseTrait::Speed);
        }
        "Viperidae" | "Elapidae" | "Buthidae" => {
            diet = Diet::Carnivore;
            defenses.insert(DefenseTrait::Venom);
        }
        "Octopodidae" => {
            habitat = Habitat::Marine;
            diet = Diet::Carnivore;
            defenses.insert(DefenseTrait::Ink);
            defenses.insert(DefenseTrait::Camouflage);
        }
        _ => {}
    }

    InferredTaxonomyDefaults {
        phylum,
        taxonomic_class,
        habitat,
        diet,
        average_mass_grams: mass_g,
        defenses,
    }
}

pub(crate) fn infer_almanac_category(taxonomic_class: &str, habitat: Habitat) -> &'static str {
    if matches!(habitat, Habitat::Marine | Habitat::Abyssal)
        && !matches!(taxonomic_class, "Aves" | "Reptilia")
    {
        return "aquatic";
    }
    match taxonomic_class {
        "Aves" => "birds",
        "Mammalia" => "mammals",
        "Reptilia" | "Amphibia" => "reptiles_amphibians",
        "Insecta" | "Arachnida" => "insects_arachnids",
        _ => "aquatic",
    }
}

pub(crate) fn clean_opt(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn serialize_slug<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|val| val.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

pub(crate) fn parse_diet(raw: &str) -> Option<Diet> {
    match raw {
        "carnivore" => Some(Diet::Carnivore),
        "herbivore" => Some(Diet::Herbivore),
        "omnivore" => Some(Diet::Omnivore),
        "detritivore" => Some(Diet::Detritivore),
        _ => None,
    }
}

pub(crate) fn parse_habitat(raw: &str) -> Option<Habitat> {
    match raw {
        "abyssal" => Some(Habitat::Abyssal),
        "marine" => Some(Habitat::Marine),
        "freshwater" => Some(Habitat::Freshwater),
        "terrestrial" => Some(Habitat::Terrestrial),
        "aerial" => Some(Habitat::Aerial),
        _ => None,
    }
}

pub(crate) fn parse_iucn(raw: &str) -> IucnStatus {
    match raw.trim().to_ascii_uppercase().as_str() {
        "LC" | "LEAST_CONCERN" => IucnStatus::Lc,
        "NT" | "NEAR_THREATENED" => IucnStatus::Nt,
        "VU" | "VULNERABLE" => IucnStatus::Vu,
        "EN" | "ENDANGERED" => IucnStatus::En,
        "CR" | "CRITICALLY_ENDANGERED" => IucnStatus::Cr,
        _ => IucnStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_client_rolls_up_subspecies_to_base_species() {
        let client = MockVisionClient::default_cached(0.35);
        let ident = client
            .identify(
                b"fake-jpg",
                GeoPoint {
                    latitude: 19.19,
                    longitude: -96.13,
                    accuracy_meters: 5.0,
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(ident.taxon_id, TaxonId::INaturalist(41_967));
        assert_eq!(ident.scientific_name, "Panthera tigris");
        assert_eq!(ident.common_name.as_deref(), Some("Tigre"));
        assert_eq!(ident.rank, TaxonomicRank::Species);
        assert_eq!(
            ident.subspecies_label.as_deref(),
            Some("Panthera tigris tigris")
        );
        assert!((ident.confidence - 0.945).abs() < 0.001);
    }

    #[tokio::test]
    async fn mock_client_uses_taxa_lookup_json_when_ancestors_omitted_and_rejects_genus() {
        let cv_subspecies_no_ancestors = r#"{
            "results": [{
                "combined_score": 88.0,
                "taxon": {
                    "id": 130793,
                    "name": "Panthera tigris tigris",
                    "rank": "subspecies",
                    "rank_level": 5
                }
            }]
        }"#;
        let taxa_json = r#"{
            "results": [{
                "id": 130793,
                "name": "Panthera tigris tigris",
                "rank": "subspecies",
                "rank_level": 5,
                "ancestors": [
                    { "id": 41962, "name": "Panthera", "rank": "genus", "rank_level": 20 },
                    { "id": 41967, "name": "Panthera tigris", "rank": "species", "rank_level": 10, "preferred_common_name": "Tigre" }
                ]
            }]
        }"#;

        let client = MockVisionClient::from_cached_json(
            cv_subspecies_no_ancestors,
            Some(taxa_json.to_string()),
            0.35,
        );
        let ident = client
            .identify(
                b"img",
                GeoPoint {
                    latitude: 19.0,
                    longitude: -96.0,
                    accuracy_meters: 5.0,
                },
                None,
            )
            .await
            .unwrap();
        assert_eq!(ident.taxon_id, TaxonId::INaturalist(41_967));
        assert_eq!(
            ident.subspecies_label.as_deref(),
            Some("Panthera tigris tigris")
        );

        let genus_only_cv = r#"{
            "results": [{
                "combined_score": 91.0,
                "taxon": {
                    "id": 41962,
                    "name": "Panthera",
                    "rank": "genus",
                    "rank_level": 20
                }
            }]
        }"#;
        let broad_client = MockVisionClient::from_cached_json(genus_only_cv, None, 0.35);
        let err = broad_client
            .identify(
                b"img",
                GeoPoint {
                    latitude: 19.0,
                    longitude: -96.0,
                    accuracy_meters: 5.0,
                },
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            VisionError::TaxonomicRankTooBroad {
                ref detected_name,
                ref detected_rank
            } if detected_name == "Panthera" && detected_rank == "genus"
        ));
    }

    #[test]
    fn parses_inaturalist_gbif_and_wikidata_enrichment_facts() {
        let inat_json = r#"{
            "results": [{
                "id": 41970,
                "name": "Panthera onca",
                "preferred_common_name": "Jaguar",
                "iconic_taxon_name": "Mammalia",
                "conservation_status": { "status": "NT" }
            }]
        }"#;
        let inat_traits =
            parse_inaturalist_taxon_response(inat_json, TaxonId::INaturalist(41_970)).unwrap();
        assert_eq!(inat_traits.iucn_status, IucnStatus::Nt);
        assert_eq!(inat_traits.habitat, Habitat::Terrestrial);

        let gbif_match_json = r#"{
            "usageKey": 5219416,
            "speciesKey": 5219416,
            "canonicalName": "Panthera tigris",
            "phylum": "Chordata",
            "class": "Mammalia",
            "order": "Carnivora",
            "family": "Felidae",
            "matchType": "EXACT"
        }"#;
        let gbif_match = parse_gbif_match_response(gbif_match_json).unwrap().unwrap();
        assert_eq!(gbif_match.order.as_deref(), Some("Carnivora"));
        assert_eq!(
            parse_gbif_iucn_response(r#"{"code":"EN","category":"ENDANGERED"}"#),
            Some(IucnStatus::En)
        );

        let wikidata_json = r#"{
            "entities": {
                "Q19939": {
                    "labels": { "es": { "value": "Tigre" } },
                    "descriptions": { "es": { "value": "especie de mamífero" } },
                    "claims": {
                        "P225": [{ "mainsnak": { "datavalue": { "value": "Panthera tigris" } } }],
                        "P141": [{ "mainsnak": { "datavalue": { "value": { "id": "Q11394" } } } }],
                        "P2067": [
                            { "mainsnak": { "datavalue": { "value": { "amount": "+1.19", "unit": "http://www.wikidata.org/entity/Q11570" } } } },
                            { "mainsnak": { "datavalue": { "value": { "amount": "+180", "unit": "http://www.wikidata.org/entity/Q11570" } } } },
                            { "mainsnak": { "datavalue": { "value": { "amount": "+120", "unit": "http://www.wikidata.org/entity/Q11570" } } } }
                        ]
                    }
                }
            }
        }"#;
        let wd = parse_wikidata_entities_response(wikidata_json, "Panthera tigris").unwrap();
        // Filters out 1.19 kg birth weight and averages 180 kg & 120 kg -> 150_000 g.
        assert_eq!(wd.mass_grams, Some(150_000));
        assert_eq!(wd.iucn_status, Some(IucnStatus::En));
        assert_eq!(wd.label_es.as_deref(), Some("Tigre"));
    }

    #[test]
    fn filters_non_fauna_candidates_and_rejects_low_percentage_scores() {
        // Low score 0.45% must be rejected even though 0.45 > 0.35 numerically.
        let low_pct = r#"{
            "results": [{
                "combined_score": 0.45,
                "taxon": { "id": 13858, "name": "Passer domesticus", "rank": "species", "rank_level": 10, "iconic_taxon_name": "Aves" }
            }]
        }"#;
        let low_err = parse_inaturalist_cv_candidate(low_pct, 0.35).unwrap_err();
        assert!(matches!(low_err, VisionError::NoSpeciesDetected));

        // Plant top result skipped when a qualifying butterfly result follows.
        let mixed_plant_then_insect = r#"{
            "results": [
                {
                    "combined_score": 82.0,
                    "taxon": { "id": 51111, "name": "Asclepias curassavica", "rank": "species", "rank_level": 10, "iconic_taxon_name": "Plantae" }
                },
                {
                    "combined_score": 68.0,
                    "taxon": { "id": 48662, "name": "Danaus plexippus", "rank": "species", "rank_level": 10, "iconic_taxon_name": "Insecta", "preferred_common_name": "Mariposa Monarca" }
                }
            ]
        }"#;
        let picked = parse_inaturalist_cv_candidate(mixed_plant_then_insect, 0.35).unwrap();
        assert_eq!(picked.taxon.id, 48_662);

        // Only plant detected -> NonFaunaDetected error.
        let plant_only = r#"{
            "results": [{
                "combined_score": 90.0,
                "taxon": { "id": 51111, "name": "Monstera deliciosa", "rank": "species", "rank_level": 10, "iconic_taxon_name": "Plantae" }
            }]
        }"#;
        let plant_err = parse_inaturalist_cv_candidate(plant_only, 0.35).unwrap_err();
        assert!(matches!(plant_err, VisionError::NonFaunaDetected { .. }));
    }
}
