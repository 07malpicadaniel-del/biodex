//! Taxonomic species identification and biological trait resolution (iNaturalist / GBIF + catalog cache).

use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;

use biodex_core::crypto::GeoPoint;
use biodex_core::taxonomy::{BiologicalTraits, DefenseTrait, Diet, Habitat, IucnStatus, TaxonId};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use thiserror::Error;

use crate::geo::taxon_db_parts;

const INAT_CV_ENDPOINT: &str = "https://api.inaturalist.org/v1/computervision/score_image";
const INAT_TAXA_BASE: &str = "https://api.inaturalist.org/v1/taxa";
const GBIF_SPECIES_BASE: &str = "https://api.gbif.org/v1/species";

/// Result returned by a taxonomic vision classifier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisionIdentification {
    /// Matched external taxon ID.
    pub taxon_id: TaxonId,
    /// Binomial scientific name.
    pub scientific_name: String,
    /// Optional vernacular name.
    pub common_name: Option<String>,
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

/// Errors produced during species vision classification or metadata enrichment.
#[derive(Debug, Error)]
pub enum VisionError {
    /// No vision API token is configured and dev hints are disabled.
    #[error("taxonomic vision classifier is not configured (set INATURALIST_API_TOKEN)")]
    ClassifierNotConfigured,
    /// Vision model returned no species candidate.
    #[error("no taxonomic match found in captured image")]
    NoSpeciesDetected,
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
    /// Identifies the animal species present in raw/JPEG image bytes.
    fn identify<'a>(
        &'a self,
        image_bytes: &'a [u8],
        coords: GeoPoint,
        dev_hint: Option<&'a DevTaxonHint>,
    ) -> Pin<Box<dyn Future<Output = Result<VisionIdentification, VisionError>> + Send + 'a>>;
}

/// Production classifier backed by iNaturalist Computer Vision API (with optional dev hint fallback).
#[derive(Debug, Clone)]
pub struct INaturalistVisionClient {
    http: reqwest::Client,
    api_token: Option<String>,
    allow_dev_taxon_hint: bool,
}

impl INaturalistVisionClient {
    /// Creates a new [`INaturalistVisionClient`].
    #[must_use]
    pub fn new(
        http: reqwest::Client,
        api_token: Option<String>,
        allow_dev_taxon_hint: bool,
    ) -> Self {
        Self {
            http,
            api_token,
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
                    .text("lng", coords.longitude.to_string());

                let resp = self
                    .http
                    .post(INAT_CV_ENDPOINT)
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
                return parse_inaturalist_cv_response(&body);
            }

            if self.allow_dev_taxon_hint
                && let Some(hint) = dev_hint
            {
                let confidence = hint.confidence.unwrap_or(0.95).clamp(0.0, 1.0);
                return Ok(VisionIdentification {
                    taxon_id: hint.taxon_id,
                    scientific_name: String::new(),
                    common_name: None,
                    confidence,
                });
            }

            Err(VisionError::ClassifierNotConfigured)
        })
    }
}

/// Resolves full [`BiologicalTraits`] from `species_catalog` or caches them from public iNaturalist / GBIF APIs.
pub async fn resolve_biological_traits(
    pool: &PgPool,
    http: &reqwest::Client,
    ident: &VisionIdentification,
) -> Result<BiologicalTraits, VisionError> {
    if let Some(cached) = fetch_cached_traits(pool, ident.taxon_id).await? {
        return Ok(cached);
    }

    let fetched = match ident.taxon_id {
        TaxonId::INaturalist(id) => fetch_inaturalist_taxon(http, id).await?,
        TaxonId::Gbif(key) => fetch_gbif_taxon(http, key).await?,
    };

    cache_traits(pool, &fetched).await?;
    Ok(fetched)
}

async fn fetch_cached_traits(
    pool: &PgPool,
    taxon_id: TaxonId,
) -> Result<Option<BiologicalTraits>, VisionError> {
    let (taxon_source, taxon_external_id) =
        taxon_db_parts(taxon_id).map_err(|_| VisionError::UnknownTaxon(taxon_id))?;

    let row = sqlx::query(
        r#"
        SELECT scientific_name, common_name, phylum, taxonomic_class,
               average_mass_grams, diet, habitat, defenses, iucn_status
        FROM species_catalog
        WHERE taxon_source = $1 AND taxon_external_id = $2
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
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

    Ok(Some(BiologicalTraits {
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
    }))
}

async fn cache_traits(pool: &PgPool, traits: &BiologicalTraits) -> Result<(), VisionError> {
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
            phylum, taxonomic_class, average_mass_grams, diet, habitat, defenses, iucn_status
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        ON CONFLICT (taxon_source, taxon_external_id) DO NOTHING
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
    .execute(pool)
    .await?;

    Ok(())
}

async fn fetch_inaturalist_taxon(
    http: &reqwest::Client,
    taxon_id: u64,
) -> Result<BiologicalTraits, VisionError> {
    let url = format!("{INAT_TAXA_BASE}/{taxon_id}");
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
    parse_inaturalist_taxon_response(&body, TaxonId::INaturalist(taxon_id))
}

async fn fetch_gbif_taxon(
    http: &reqwest::Client,
    taxon_key: u64,
) -> Result<BiologicalTraits, VisionError> {
    let url = format!("{GBIF_SPECIES_BASE}/{taxon_key}");
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

#[derive(Deserialize)]
struct INatCvResponse {
    results: Vec<INatCvResult>,
}

#[derive(Deserialize)]
struct INatCvResult {
    combined_score: Option<f32>,
    vision_score: Option<f32>,
    taxon: INatTaxon,
}

#[derive(Deserialize)]
struct INatTaxaResponse {
    results: Vec<INatTaxon>,
}

#[derive(Deserialize)]
struct INatTaxon {
    id: u64,
    name: String,
    preferred_common_name: Option<String>,
    iconic_taxon_name: Option<String>,
    conservation_status: Option<INatConservationStatus>,
}

#[derive(Deserialize)]
struct INatConservationStatus {
    status: Option<String>,
}

#[derive(Deserialize)]
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

/// Parses an iNaturalist computer vision JSON payload into [`VisionIdentification`].
pub fn parse_inaturalist_cv_response(raw_json: &str) -> Result<VisionIdentification, VisionError> {
    let parsed: INatCvResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    let top = parsed
        .results
        .into_iter()
        .next()
        .ok_or(VisionError::NoSpeciesDetected)?;

    let raw_score = top.combined_score.or(top.vision_score).unwrap_or(0.0);
    let confidence = if raw_score > 1.0 {
        (raw_score / 100.0).clamp(0.0, 1.0)
    } else {
        raw_score.clamp(0.0, 1.0)
    };

    Ok(VisionIdentification {
        taxon_id: TaxonId::INaturalist(top.taxon.id),
        scientific_name: top.taxon.name,
        common_name: top.taxon.preferred_common_name,
        confidence,
    })
}

/// Parses an iNaturalist `/v1/taxa/{id}` JSON response into default [`BiologicalTraits`].
pub fn parse_inaturalist_taxon_response(
    raw_json: &str,
    taxon_id: TaxonId,
) -> Result<BiologicalTraits, VisionError> {
    let parsed: INatTaxaResponse =
        serde_json::from_str(raw_json).map_err(|err| VisionError::Upstream(err.to_string()))?;
    let taxon = parsed
        .results
        .into_iter()
        .next()
        .ok_or(VisionError::UnknownTaxon(taxon_id))?;

    let iconic = taxon.iconic_taxon_name.as_deref().unwrap_or("Animalia");
    let (phylum, taxonomic_class, habitat, diet, average_mass_grams) =
        infer_defaults_from_iconic(iconic);
    let iucn_status = taxon
        .conservation_status
        .and_then(|cs| cs.status)
        .map(|code| parse_iucn(&code))
        .unwrap_or(IucnStatus::Unknown);

    Ok(BiologicalTraits {
        taxon_id,
        scientific_name: taxon.name,
        common_name: taxon.preferred_common_name,
        phylum: phylum.to_string(),
        taxonomic_class: taxonomic_class.to_string(),
        average_mass_grams,
        diet,
        habitat,
        defenses: BTreeSet::new(),
        iucn_status,
    })
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
    let (_, _, habitat, diet, average_mass_grams) = infer_defaults_from_iconic(&taxonomic_class);

    Ok(BiologicalTraits {
        taxon_id,
        scientific_name,
        common_name: parsed.vernacular_name,
        phylum,
        taxonomic_class,
        average_mass_grams,
        diet,
        habitat,
        defenses: BTreeSet::new(),
        iucn_status: IucnStatus::Unknown,
    })
}

fn infer_defaults_from_iconic(iconic: &str) -> (&'static str, &'static str, Habitat, Diet, u64) {
    match iconic {
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
        "Actinopterygii" => (
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
        _ => (
            "Animalia",
            "Unknown",
            Habitat::Terrestrial,
            Diet::Omnivore,
            500,
        ),
    }
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
        "LC" => IucnStatus::Lc,
        "NT" => IucnStatus::Nt,
        "VU" => IucnStatus::Vu,
        "EN" => IucnStatus::En,
        "CR" => IucnStatus::Cr,
        _ => IucnStatus::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inaturalist_cv_top_result() {
        let json = r#"{
            "results": [
                {
                    "combined_score": 93.4,
                    "taxon": {
                        "id": 14824,
                        "name": "Passer domesticus",
                        "preferred_common_name": "House Sparrow",
                        "iconic_taxon_name": "Aves"
                    }
                }
            ]
        }"#;

        let ident = parse_inaturalist_cv_response(json).unwrap();
        assert_eq!(ident.taxon_id, TaxonId::INaturalist(14_824));
        assert_eq!(ident.scientific_name, "Passer domesticus");
        assert!((ident.confidence - 0.934).abs() < 0.001);
    }

    #[test]
    fn parses_inaturalist_and_gbif_taxon_metadata() {
        let inat_json = r#"{
            "results": [
                {
                    "id": 41970,
                    "name": "Panthera onca",
                    "preferred_common_name": "Jaguar",
                    "iconic_taxon_name": "Mammalia",
                    "conservation_status": { "status": "NT" }
                }
            ]
        }"#;
        let inat_traits =
            parse_inaturalist_taxon_response(inat_json, TaxonId::INaturalist(41_970)).unwrap();
        assert_eq!(inat_traits.iucn_status, IucnStatus::Nt);
        assert_eq!(inat_traits.habitat, Habitat::Terrestrial);

        let gbif_json = r#"{
            "canonicalName": "Architeuthis dux",
            "phylum": "Mollusca",
            "class": "Cephalopoda"
        }"#;
        let gbif_traits = parse_gbif_taxon_response(gbif_json, TaxonId::Gbif(5_189_893)).unwrap();
        assert_eq!(gbif_traits.scientific_name, "Architeuthis dux");
        assert_eq!(gbif_traits.phylum, "Mollusca");
    }
}
