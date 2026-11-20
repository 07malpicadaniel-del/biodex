//! Axum HTTP router and request/response handlers for Phase 1 MVP.

use std::sync::Arc;

use axum::extract::{DefaultBodyLimit, Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use biodex_core::crypto::{
    CaptureMetadata, CaptureSource, GeoPoint, ImuReading, verify_competitive_capture,
};
use biodex_core::mods::ModRegistry;
use biodex_core::taxonomy::{CaptureVariant, SpeciesCard, generate_card};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use tower_http::trace::TraceLayer;
use uuid::Uuid;

use crate::almanac::{
    AlmanacError, AlmanacFilter, AlmanacItemDto, AlmanacListResponse, NaturaListaDiscoveryResponse,
    NaturaListaSyncOptions, NaturaListaSyncReport, get_almanac_entry, inspect_naturalista_taxon,
    list_almanac, nearby_naturalista_fauna, search_naturalista_fauna,
    sync_naturalista_clade_species,
};
use crate::config::TaxonomyEndpoints;
use crate::error::ApiError;
use crate::geo::{
    GeoClassification, classify_capture_location_with_naturalista, parse_taxon_parts,
    taxon_db_parts,
};
use crate::vision::{
    DevTaxonHint, TaxonomyClassifier, VisionError, VisionIdentification, naturalista_url_for_taxon,
    resolve_species_profile, serialize_slug,
};

const MAX_BODY_BYTES: usize = 10 * 1024 * 1024;

/// Shared application state injected into Axum handlers.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL + PostGIS connection pool.
    pub db: PgPool,
    /// Shared HTTP client for upstream taxonomy lookups.
    pub http: reqwest::Client,
    /// Loaded layered mod registry (`base-pack` + community packs).
    pub mods: Arc<ModRegistry>,
    /// Taxonomic vision identification provider.
    pub vision: Arc<dyn TaxonomyClassifier>,
    /// External taxonomy/enrichment endpoints (iNaturalist, GBIF, Wikidata).
    pub taxonomy_endpoints: TaxonomyEndpoints,
    /// Optional `.env` iNaturalist / NaturaLista API token for authenticated taxa requests.
    pub inaturalist_api_token: Option<String>,
    /// Maximum allowed anti-Moiré screen detection score.
    pub max_moire_score: f32,
    /// Enables regional NaturaLista occurrence fallback when a species lacks curated PostGIS ranges.
    pub enable_live_range_fallback: bool,
}

/// Incoming JSON payload for `POST /api/v1/captures`.
#[derive(Debug, Deserialize)]
pub struct CaptureUploadRequest {
    /// Optional player identifier for MVP capture attribution.
    pub user_id: Option<String>,
    /// Base64-encoded JPEG/RAW camera frame bytes.
    pub image_base64: String,
    /// Signed capture telemetry metadata.
    pub metadata: CaptureMetadataDto,
    /// Optional dev-only taxon hint (ignored unless `ALLOW_DEV_TAXON_HINT=true`).
    pub dev_taxon_hint: Option<DevTaxonHint>,
}

/// JSON DTO representation of [`CaptureMetadata`] with base64-encoded binary fields.
#[derive(Debug, Deserialize)]
pub struct CaptureMetadataDto {
    /// Client-generated UUID string.
    pub capture_id: String,
    /// Capture timestamp in UTC milliseconds since epoch.
    pub captured_at_utc_ms: i64,
    /// Image pipeline source (`camera_x_live` or `gallery`).
    pub source: CaptureSource,
    /// GPS coordinates at shutter release.
    pub coordinates: GeoPoint,
    /// Accelerometer & gyroscope telemetry.
    pub imu: ImuReading,
    /// Client anti-Moiré heuristic score (`0.0..=1.0`).
    pub moire_score: f32,
    /// Base64-encoded 32-byte SHA-256 digest of the raw/JPEG image.
    pub image_sha256_base64: String,
    /// Base64-encoded ECDSA P-256 public key (SPKI DER or SEC1).
    pub device_public_key_base64: String,
    /// Base64-encoded ECDSA P-256 signature (ASN.1 DER or raw `r||s`).
    pub signature_base64: String,
}

/// Response returned by `GET /health`.
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Overall service status.
    pub status: String,
    /// Database reachability status.
    pub database: String,
    /// Installed PostGIS version string.
    pub postgis_version: String,
}

/// Response returned by `POST /api/v1/captures`.
#[derive(Debug, Serialize, Deserialize)]
pub struct CaptureUploadResponse {
    /// `"minted"` when valid in a wild range or sanctuary; `"discarded"` when out of range.
    pub status: String,
    /// Database primary key (`captures.id`).
    pub capture_record_id: Uuid,
    /// Client-supplied unique `capture_id`.
    pub capture_id: String,
    /// PostGIS zone classification outcome.
    pub classification: GeoClassification,
    /// Taxonomic vision result (enriched with scientific name when resolved).
    pub identification: VisionIdentification,
    /// Generated creature card (`None` when discarded).
    pub card: Option<SpeciesCard>,
    /// Canonical NaturaLista / GBIF reference page URL.
    #[serde(default)]
    pub naturalista_url: Option<String>,
    /// Reference photograph URL from NaturaLista (when available).
    #[serde(default)]
    pub image_url: Option<String>,
    /// Taxonomic class label (`Aves`, `Mammalia`, etc.).
    #[serde(default)]
    pub taxonomic_class: Option<String>,
    /// Biological habitat slug (`terrestrial`, `aerial`, etc.).
    #[serde(default)]
    pub habitat: Option<String>,
    /// IUCN Red List code (`LC`, `VU`, `CR`, etc.).
    #[serde(default)]
    pub iucn_status: Option<String>,
    /// Estimated average adult body mass in grams.
    #[serde(default)]
    pub average_mass_grams: Option<u64>,
}

/// Builds the Phase 1 Axum [`Router`] with body limits and HTTP tracing.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/captures", post(submit_capture_handler))
        .route("/api/v1/almanac", get(list_almanac_handler))
        .route(
            "/api/v1/almanac/naturalista/search",
            get(search_naturalista_handler),
        )
        .route(
            "/api/v1/almanac/naturalista/nearby",
            get(nearby_naturalista_handler),
        )
        .route(
            "/api/v1/almanac/naturalista/sync",
            post(sync_naturalista_handler),
        )
        .route(
            "/api/v1/almanac/naturalista/taxon/{id}",
            get(inspect_naturalista_handler),
        )
        .route(
            "/api/v1/almanac/{source}/{taxon_id}",
            get(get_almanac_entry_handler),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct AlmanacEntryQuery {
    user_id: Option<String>,
    #[serde(default)]
    dev_mode: bool,
}

#[derive(Debug, Deserialize)]
struct NaturaListaSearchQuery {
    q: String,
    user_id: Option<String>,
    #[serde(default)]
    dev_mode: bool,
}

#[derive(Debug, Deserialize)]
struct NaturaListaNearbyQuery {
    lat: f64,
    lng: f64,
    radius_km: Option<u32>,
    user_id: Option<String>,
    #[serde(default)]
    dev_mode: bool,
}

async fn list_almanac_handler(
    State(state): State<AppState>,
    Query(filter): Query<AlmanacFilter>,
) -> Result<Json<AlmanacListResponse>, ApiError> {
    let response = list_almanac(&state.db, &state.mods, &filter)
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?;
    Ok(Json(response))
}

async fn search_naturalista_handler(
    State(state): State<AppState>,
    Query(query): Query<NaturaListaSearchQuery>,
) -> Result<Json<NaturaListaDiscoveryResponse>, ApiError> {
    let response = search_naturalista_fauna(
        &state.db,
        &state.http,
        &state.taxonomy_endpoints.inaturalist_base_url,
        &query.q,
        query.user_id.as_deref(),
        query.dev_mode,
    )
    .await
    .map_err(map_almanac_error)?;
    Ok(Json(response))
}

async fn nearby_naturalista_handler(
    State(state): State<AppState>,
    Query(query): Query<NaturaListaNearbyQuery>,
) -> Result<Json<NaturaListaDiscoveryResponse>, ApiError> {
    let coords = GeoPoint {
        latitude: query.lat,
        longitude: query.lng,
        accuracy_meters: 25.0,
    };
    let response = nearby_naturalista_fauna(
        &state.db,
        &state.http,
        &state.taxonomy_endpoints.inaturalist_base_url,
        coords,
        query.radius_km.unwrap_or(25),
        query.user_id.as_deref(),
        query.dev_mode,
    )
    .await
    .map_err(map_almanac_error)?;
    Ok(Json(response))
}

async fn sync_naturalista_handler(
    State(state): State<AppState>,
    Query(options): Query<NaturaListaSyncOptions>,
) -> Result<Json<NaturaListaSyncReport>, ApiError> {
    let report = sync_naturalista_clade_species(
        &state.db,
        &state.http,
        &state.taxonomy_endpoints.inaturalist_base_url,
        state.inaturalist_api_token.as_deref(),
        &options,
    )
    .await
    .map_err(map_almanac_error)?;
    Ok(Json(report))
}

async fn inspect_naturalista_handler(
    State(state): State<AppState>,
    AxumPath(inat_id): AxumPath<u64>,
    Query(query): Query<AlmanacEntryQuery>,
) -> Result<Json<AlmanacItemDto>, ApiError> {
    let entry = inspect_naturalista_taxon(
        &state.db,
        &state.http,
        &state.mods,
        &state.taxonomy_endpoints,
        inat_id,
        query.user_id.as_deref(),
        query.dev_mode,
    )
    .await
    .map_err(map_almanac_error)?
    .ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "naturalista_taxon_not_found",
            "species not found on NaturaLista",
        )
    })?;
    Ok(Json(entry))
}

async fn get_almanac_entry_handler(
    State(state): State<AppState>,
    AxumPath((source, taxon_external_id)): AxumPath<(String, i64)>,
    Query(query): Query<AlmanacEntryQuery>,
) -> Result<Json<AlmanacItemDto>, ApiError> {
    let taxon_id = parse_taxon_parts(&source, taxon_external_id).ok_or_else(|| {
        ApiError::bad_request(
            "invalid_taxon_id",
            "source must be 'gbif' or 'i_naturalist' with positive numeric id",
        )
    })?;

    let entry = get_almanac_entry(
        &state.db,
        &state.mods,
        taxon_id,
        query.user_id.as_deref(),
        query.dev_mode,
    )
    .await
    .map_err(map_almanac_error)?
    .ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_FOUND,
            "almanac_species_not_found",
            "species not found in almanac catalog",
        )
    })?;

    Ok(Json(entry))
}

async fn health_handler(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let row = sqlx::query("SELECT PostGIS_Version() AS postgis_version")
        .fetch_one(&state.db)
        .await
        .map_err(|err| {
            tracing::error!(error = %err, "database health check failed");
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "database_unavailable",
                "PostgreSQL / PostGIS is unavailable",
            )
        })?;

    let postgis_version: String = row
        .try_get("postgis_version")
        .map_err(|err| ApiError::internal(err.to_string()))?;

    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        database: "ok".to_string(),
        postgis_version,
    }))
}

async fn submit_capture_handler(
    State(state): State<AppState>,
    Json(req): Json<CaptureUploadRequest>,
) -> Result<(StatusCode, Json<CaptureUploadResponse>), ApiError> {
    let image_bytes = decode_base64_field(&req.image_base64, "image_base64")?;
    if image_bytes.is_empty() {
        return Err(ApiError::bad_request(
            "empty_image",
            "captured image payload must not be empty",
        ));
    }

    let meta = build_capture_metadata(req.metadata)?;
    let computed_digest: [u8; 32] = Sha256::digest(&image_bytes).into();
    if computed_digest != meta.image_sha256 {
        return Err(ApiError::bad_request(
            "image_hash_mismatch",
            "uploaded image SHA-256 does not match signed metadata",
        ));
    }

    verify_competitive_capture(&meta, state.max_moire_score)?;

    let captured_at_utc = DateTime::<Utc>::from_timestamp_millis(meta.captured_at_utc_ms)
        .ok_or_else(|| ApiError::bad_request("invalid_timestamp", "timestamp out of UTC range"))?;

    let already_exists = sqlx::query("SELECT 1 FROM captures WHERE capture_id = $1")
        .bind(&meta.capture_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|err| ApiError::internal(err.to_string()))?;
    if already_exists.is_some() {
        return Err(ApiError::conflict(
            "duplicate_capture_id",
            "capture_id has already been submitted",
        ));
    }

    let mut identification = state
        .vision
        .identify(&image_bytes, meta.coordinates, req.dev_taxon_hint.as_ref())
        .await
        .map_err(map_vision_error)?;

    let profile = resolve_species_profile(
        &state.db,
        &state.http,
        &identification,
        &state.taxonomy_endpoints,
    )
    .await
    .map_err(map_vision_error)?;
    let traits = profile.traits;

    if identification.scientific_name.is_empty() {
        identification.scientific_name = traits.scientific_name.clone();
    }
    if identification.common_name.is_none() {
        identification.common_name = traits.common_name.clone();
    }

    let geo_resolution = classify_capture_location_with_naturalista(
        &state.db,
        &state.http,
        &state.taxonomy_endpoints.inaturalist_base_url,
        meta.coordinates,
        identification.taxon_id,
        state.enable_live_range_fallback,
    )
    .await
    .map_err(|err| ApiError::internal(err.to_string()))?;

    let (zone_str, card_opt) = match &geo_resolution.classification {
        GeoClassification::Sanctuary { .. } => {
            let base_card = generate_card(&traits, CaptureVariant::Sanctuary)?;
            let final_card = state.mods.apply_to_card(base_card);
            ("sanctuary", Some(final_card))
        }
        GeoClassification::Wild => {
            let base_card = generate_card(&traits, CaptureVariant::Wild)?;
            let final_card = state.mods.apply_to_card(base_card);
            ("wild", Some(final_card))
        }
        GeoClassification::OutOfRange => ("discarded", None),
    };

    let (taxon_source, taxon_external_id) = taxon_db_parts(identification.taxon_id)
        .map_err(|err| ApiError::bad_request("invalid_taxon_id", err.to_string()))?;

    let card_json = card_opt
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|err| ApiError::internal(err.to_string()))?;

    let record_id = insert_capture_record(
        &state.db,
        CaptureInsert {
            capture_id: &meta.capture_id,
            user_id: req.user_id.as_deref(),
            device_public_key: &meta.device_public_key,
            image_sha256: &meta.image_sha256,
            captured_at_utc,
            coordinates: meta.coordinates,
            zone_classification: zone_str,
            sanctuary_zone_id: geo_resolution.sanctuary_zone_id,
            geo_zone_id: geo_resolution.geo_zone_id,
            taxon_source,
            taxon_external_id,
            subspecies_label: identification.subspecies_label.as_deref(),
            vision_confidence: identification.confidence,
            generated_card: card_json,
        },
    )
    .await?;

    let (status_code, status_label) = if card_opt.is_some() {
        (StatusCode::CREATED, "minted".to_string())
    } else {
        (StatusCode::OK, "discarded".to_string())
    };

    let iucn_str = serialize_slug(&traits.iucn_status);
    let habitat_str = serialize_slug(&traits.habitat);
    let naturalista_url = naturalista_url_for_taxon(identification.taxon_id);

    Ok((
        status_code,
        Json(CaptureUploadResponse {
            status: status_label,
            capture_record_id: record_id,
            capture_id: meta.capture_id,
            classification: geo_resolution.classification,
            identification,
            card: card_opt,
            naturalista_url: Some(naturalista_url),
            image_url: profile.media.image_url,
            taxonomic_class: Some(traits.taxonomic_class),
            habitat: Some(habitat_str),
            iucn_status: Some(iucn_str),
            average_mass_grams: Some(traits.average_mass_grams),
        }),
    ))
}

struct CaptureInsert<'a> {
    capture_id: &'a str,
    user_id: Option<&'a str>,
    device_public_key: &'a [u8],
    image_sha256: &'a [u8],
    captured_at_utc: DateTime<Utc>,
    coordinates: GeoPoint,
    zone_classification: &'a str,
    sanctuary_zone_id: Option<Uuid>,
    geo_zone_id: Option<Uuid>,
    taxon_source: &'a str,
    taxon_external_id: i64,
    subspecies_label: Option<&'a str>,
    vision_confidence: f32,
    generated_card: Option<serde_json::Value>,
}

async fn insert_capture_record(pool: &PgPool, row: CaptureInsert<'_>) -> Result<Uuid, ApiError> {
    let result = sqlx::query(
        r#"
        INSERT INTO captures (
            capture_id, user_id, device_public_key, image_sha256, captured_at_utc,
            location, zone_classification, sanctuary_zone_id, geo_zone_id,
            taxon_source, taxon_external_id, subspecies_label, vision_confidence, generated_card
        )
        VALUES (
            $1, $2, $3, $4, $5,
            ST_SetSRID(ST_MakePoint($6, $7), 4326)::geography, $8, $9, $10,
            $11, $12, $13, $14, $15
        )
        RETURNING id
        "#,
    )
    .bind(row.capture_id)
    .bind(row.user_id)
    .bind(row.device_public_key)
    .bind(row.image_sha256)
    .bind(row.captured_at_utc)
    .bind(row.coordinates.longitude)
    .bind(row.coordinates.latitude)
    .bind(row.zone_classification)
    .bind(row.sanctuary_zone_id)
    .bind(row.geo_zone_id)
    .bind(row.taxon_source)
    .bind(row.taxon_external_id)
    .bind(row.subspecies_label)
    .bind(row.vision_confidence)
    .bind(row.generated_card)
    .fetch_one(pool)
    .await;

    match result {
        Ok(inserted) => inserted
            .try_get("id")
            .map_err(|err| ApiError::internal(err.to_string())),
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            Err(ApiError::conflict(
                "duplicate_capture_id",
                "capture_id has already been submitted",
            ))
        }
        Err(err) => Err(ApiError::internal(err.to_string())),
    }
}

fn build_capture_metadata(dto: CaptureMetadataDto) -> Result<CaptureMetadata, ApiError> {
    let sha_bytes = decode_base64_field(&dto.image_sha256_base64, "image_sha256_base64")?;
    let image_sha256: [u8; 32] = sha_bytes.try_into().map_err(|_| {
        ApiError::bad_request("invalid_image_sha256", "image_sha256 must be 32 bytes")
    })?;
    let device_public_key =
        decode_base64_field(&dto.device_public_key_base64, "device_public_key_base64")?;
    let signature = decode_base64_field(&dto.signature_base64, "signature_base64")?;

    Ok(CaptureMetadata {
        capture_id: dto.capture_id,
        captured_at_utc_ms: dto.captured_at_utc_ms,
        source: dto.source,
        coordinates: dto.coordinates,
        imu: dto.imu,
        moire_score: dto.moire_score,
        image_sha256,
        device_public_key,
        signature,
    })
}

fn decode_base64_field(raw: &str, field_name: &str) -> Result<Vec<u8>, ApiError> {
    BASE64_STANDARD.decode(raw.trim()).map_err(|err| {
        ApiError::bad_request(
            "invalid_base64",
            format!("invalid base64 in '{field_name}': {err}"),
        )
    })
}

fn map_vision_error(err: VisionError) -> ApiError {
    match err {
        VisionError::ClassifierNotConfigured => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "vision_not_configured",
            err.to_string(),
        ),
        VisionError::NoSpeciesDetected => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "no_species_detected",
            err.to_string(),
        ),
        VisionError::NonFaunaDetected { .. } => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "non_fauna_detected",
            err.to_string(),
        ),
        VisionError::TaxonomicRankTooBroad { .. } => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "taxonomic_rank_too_broad",
            err.to_string(),
        ),
        VisionError::MissingSpeciesAncestor { .. } | VisionError::UnknownTaxon(_) => ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "taxon_unresolved",
            err.to_string(),
        ),
        VisionError::Upstream(_) => ApiError::new(
            StatusCode::BAD_GATEWAY,
            "taxonomy_upstream_error",
            err.to_string(),
        ),
        VisionError::Database(db_err) => ApiError::internal(db_err.to_string()),
    }
}

fn map_almanac_error(err: AlmanacError) -> ApiError {
    match err {
        AlmanacError::InvalidInput(msg) => ApiError::bad_request("invalid_query", msg),
        AlmanacError::Geo(geo_err) => {
            ApiError::bad_request("invalid_coordinates", geo_err.to_string())
        }
        AlmanacError::Vision(vision_err) => map_vision_error(vision_err),
        AlmanacError::Taxonomy(tax_err) => ApiError::internal(tax_err.to_string()),
        AlmanacError::Database(db_err) => ApiError::internal(db_err.to_string()),
    }
}
