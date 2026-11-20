//! End-to-end integration tests for `biodex-backend` against PostgreSQL + PostGIS.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use biodex_backend::almanac::{
    AlmanacItemDto, AlmanacListResponse, NaturaListaDiscoveryResponse, NaturaListaSyncReport,
    sync_almanac_catalog,
};
use biodex_backend::config::TaxonomyEndpoints;
use biodex_backend::load_mods_from_dir;
use biodex_backend::routes::{AppState, CaptureUploadResponse, HealthResponse, router};
use biodex_backend::run_migrations;
use biodex_backend::vision::{
    DevTaxonHint, MockVisionClient, TaxonomicRank, TaxonomyClassifier, VisionError,
    VisionIdentification,
};
use biodex_core::crypto::{CaptureMetadata, CaptureSource, GeoPoint, ImuReading};
use biodex_core::taxonomy::{CaptureVariant, TaxonId};
use p256::ecdsa::signature::Signer;
use p256::ecdsa::{Signature, SigningKey};
use p256::pkcs8::EncodePublicKey;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use uuid::Uuid;

struct MockVisionClassifier {
    taxon_id: TaxonId,
    scientific_name: &'static str,
}

impl TaxonomyClassifier for MockVisionClassifier {
    fn identify<'a>(
        &'a self,
        _image_bytes: &'a [u8],
        _coords: GeoPoint,
        _dev_hint: Option<&'a DevTaxonHint>,
    ) -> Pin<Box<dyn Future<Output = Result<VisionIdentification, VisionError>> + Send + 'a>> {
        Box::pin(async move {
            Ok(VisionIdentification {
                taxon_id: self.taxon_id,
                scientific_name: self.scientific_name.to_string(),
                common_name: None,
                rank: TaxonomicRank::Species,
                subspecies_label: None,
                confidence: 0.96,
            })
        })
    }
}

async fn setup_test_db() -> PgPool {
    let url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://biodex:biodex_dev_password@127.0.0.1:5432/biodex".to_string()
    });
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to PostGIS test database (run `docker compose up -d postgres`)");

    run_migrations(&pool).await.expect("run PostGIS migrations");
    pool
}

async fn seed_test_world(pool: &PgPool) -> (Uuid, Uuid) {
    // Seed Architeuthis dux (GBIF 5189893) in species_catalog.
    sqlx::query(
        r#"
        INSERT INTO species_catalog (
            taxon_source, taxon_external_id, scientific_name, common_name,
            phylum, taxonomic_class, average_mass_grams, diet, habitat, defenses, iucn_status
        )
        VALUES (
            'gbif', 5189893, 'Architeuthis dux', 'Calamar Gigante',
            'Mollusca', 'Cephalopoda', 275000, 'carnivore', 'abyssal', '["ink"]'::jsonb, 'LC'
        )
        ON CONFLICT (taxon_source, taxon_external_id) DO NOTHING
        "#,
    )
    .execute(pool)
    .await
    .unwrap();

    // Seed an overlapping Sanctuary zone and Wild range in isolated synthetic test coordinates (lon 1.0..2.5, lat 1.0..2.5).
    let sanctuary_id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let wild_id = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

    sqlx::query(
        r#"
        INSERT INTO sanctuary_zones (id, name, zone_type, boundary, source_attribution, verified)
        VALUES (
            $1,
            'Acuario de Prueba PostGIS',
            'aquarium',
            ST_Multi(ST_GeomFromText('POLYGON((1.10 1.10, 1.25 1.10, 1.25 1.25, 1.10 1.25, 1.10 1.10))', 4326)),
            'integration-test',
            TRUE
        )
        ON CONFLICT (id) DO UPDATE SET boundary = EXCLUDED.boundary
        "#,
    )
    .bind(sanctuary_id)
    .execute(pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO species_wild_ranges (id, taxon_source, taxon_external_id, range_geom, source_attribution)
        VALUES (
            $1,
            'gbif',
            5189893,
            ST_Multi(ST_GeomFromText('POLYGON((1.00 1.00, 2.50 1.00, 2.50 1.40, 1.00 1.40, 1.00 1.00))', 4326)),
            'integration-test'
        )
        ON CONFLICT (id) DO UPDATE SET range_geom = EXCLUDED.range_geom
        "#,
    )
    .bind(wild_id)
    .execute(pool)
    .await
    .unwrap();

    (sanctuary_id, wild_id)
}

fn build_app(pool: PgPool) -> Router {
    let mods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let mods = Arc::new(load_mods_from_dir(&mods_dir).unwrap());
    let vision = Arc::new(MockVisionClassifier {
        taxon_id: TaxonId::Gbif(5_189_893),
        scientific_name: "Architeuthis dux",
    });

    router(AppState {
        db: pool,
        http: reqwest::Client::new(),
        mods,
        vision,
        taxonomy_endpoints: TaxonomyEndpoints::default(),
        inaturalist_api_token: None,
        max_moire_score: 0.35,
        enable_live_range_fallback: false,
    })
}

fn signed_capture_body(
    capture_id: &str,
    source: CaptureSource,
    coords: GeoPoint,
    tamper_coords_after_sign: bool,
    tamper_image_after_sign: bool,
) -> String {
    let key_bytes = [0x55_u8; 32];
    let signing_key = SigningKey::from_bytes((&key_bytes).into()).unwrap();
    let public_key_der = p256::PublicKey::from(*signing_key.verifying_key())
        .to_public_key_der()
        .unwrap()
        .into_vec();

    let image_bytes = b"synthetic-jpeg-bytes-from-camerax".to_vec();
    let image_sha256: [u8; 32] = Sha256::digest(&image_bytes).into();

    let mut meta = CaptureMetadata {
        capture_id: capture_id.to_string(),
        captured_at_utc_ms: 1_795_017_600_000,
        source,
        coordinates: coords,
        imu: ImuReading {
            accel_xyz: [0.05, -0.02, 9.81],
            gyro_xyz: [0.0, 0.01, -0.01],
        },
        moire_score: 0.05,
        image_sha256,
        device_public_key: public_key_der.clone(),
        signature: Vec::new(),
    };

    let payload = meta.canonical_signing_bytes().unwrap();
    let sig: Signature = signing_key.sign(&payload);
    let sig_der = sig.to_der().as_bytes().to_vec();
    meta.signature = sig_der.clone();

    let sent_coords = if tamper_coords_after_sign {
        GeoPoint {
            latitude: coords.latitude + 1.0,
            ..coords
        }
    } else {
        coords
    };

    let sent_image = if tamper_image_after_sign {
        b"different-image-bytes".to_vec()
    } else {
        image_bytes
    };

    json!({
        "user_id": "test-explorer-1",
        "image_base64": BASE64_STANDARD.encode(sent_image),
        "metadata": {
            "capture_id": meta.capture_id,
            "captured_at_utc_ms": meta.captured_at_utc_ms,
            "source": meta.source,
            "coordinates": sent_coords,
            "imu": meta.imu,
            "moire_score": meta.moire_score,
            "image_sha256_base64": BASE64_STANDARD.encode(meta.image_sha256),
            "device_public_key_base64": BASE64_STANDARD.encode(public_key_der),
            "signature_base64": BASE64_STANDARD.encode(sig_der),
        }
    })
    .to_string()
}

#[tokio::test]
async fn health_endpoint_confirms_postgis_connection() {
    let pool = setup_test_db().await;
    let app = build_app(pool);

    let resp = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let health: HealthResponse = serde_json::from_slice(&body).unwrap();
    assert_eq!(health.status, "ok");
    assert_eq!(health.database, "ok");
    assert!(!health.postgis_version.is_empty());
}

#[tokio::test]
async fn capture_flow_enforces_sanctuary_priority_wild_discard_and_security() {
    let pool = setup_test_db().await;
    let (sanctuary_id, _) = seed_test_world(&pool).await;
    let app = build_app(pool.clone());

    // 1. Overlapping Sanctuary + Wild -> Sanctuary takes priority (201 Created, 85% stats, mod applied).
    let sanctuary_cid = format!("test-sanctuary-{}", Uuid::new_v4());
    let req_sanctuary = signed_capture_body(
        &sanctuary_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 1.19,
            longitude: 1.15,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(req_sanctuary))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let minted_sanctuary: CaptureUploadResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(minted_sanctuary.status, "minted");
    assert!(matches!(
        minted_sanctuary.classification,
        biodex_backend::geo::GeoClassification::Sanctuary { zone_id, .. } if zone_id == sanctuary_id
    ));
    let sanctuary_card = minted_sanctuary.card.unwrap();
    assert_eq!(sanctuary_card.variant, CaptureVariant::Sanctuary);
    assert_eq!(sanctuary_card.applied_pack, "base-pack");
    assert_eq!(sanctuary_card.moves[0].name, "Abrazo Abisal");

    // 2. Replay same capture_id -> 409 Conflict.
    let replay_body = signed_capture_body(
        &sanctuary_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 1.19,
            longitude: 1.15,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let replay_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(replay_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replay_resp.status(), StatusCode::CONFLICT);

    // 3. Wild range outside sanctuary -> Wild (201 Created, 100% stats > Sanctuary stats).
    let wild_cid = format!("test-wild-{}", Uuid::new_v4());
    let wild_body = signed_capture_body(
        &wild_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 1.19,
            longitude: 2.10,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let wild_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(wild_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wild_resp.status(), StatusCode::CREATED);
    let wild_bytes = axum::body::to_bytes(wild_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let minted_wild: CaptureUploadResponse = serde_json::from_slice(&wild_bytes).unwrap();
    let wild_card = minted_wild.card.unwrap();
    assert_eq!(wild_card.variant, CaptureVariant::Wild);
    assert!(wild_card.stats.hp > sanctuary_card.stats.hp);

    // 4. Out of range -> 200 OK with discarded status and no card.
    let discard_cid = format!("test-discard-{}", Uuid::new_v4());
    let discard_body = signed_capture_body(
        &discard_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 40.7128,
            longitude: -74.0060,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let discard_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(discard_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(discard_resp.status(), StatusCode::OK);
    let discard_bytes = axum::body::to_bytes(discard_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let discarded: CaptureUploadResponse = serde_json::from_slice(&discard_bytes).unwrap();
    assert_eq!(discarded.status, "discarded");
    assert!(discarded.card.is_none());

    // 5. Gallery source -> 403 Forbidden.
    let gallery_body = signed_capture_body(
        &format!("test-gallery-{}", Uuid::new_v4()),
        CaptureSource::Gallery,
        GeoPoint {
            latitude: 19.19,
            longitude: -95.50,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let gallery_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(gallery_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gallery_resp.status(), StatusCode::FORBIDDEN);

    // 6. Tampered coordinates after signing -> 401 Unauthorized.
    let tampered_coords_body = signed_capture_body(
        &format!("test-tamper-coords-{}", Uuid::new_v4()),
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 19.19,
            longitude: -95.50,
            accuracy_meters: 5.0,
        },
        true,
        false,
    );
    let tamper_coords_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(tampered_coords_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tamper_coords_resp.status(), StatusCode::UNAUTHORIZED);

    // 7. Tampered image bytes vs signed SHA-256 -> 400 Bad Request.
    let tampered_img_body = signed_capture_body(
        &format!("test-tamper-img-{}", Uuid::new_v4()),
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 19.19,
            longitude: -95.50,
            accuracy_meters: 5.0,
        },
        false,
        true,
    );
    let tamper_img_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(tampered_img_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tamper_img_resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn almanac_endpoint_lists_24_basic_species_filters_and_tracks_discovery() {
    let pool = setup_test_db().await;
    let mods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let mods = Arc::new(load_mods_from_dir(&mods_dir).unwrap());
    let synced = sync_almanac_catalog(&pool, &mods).await.unwrap();
    assert_eq!(synced, 92);

    let test_user = format!("almanac-user-{}", Uuid::new_v4());
    let app = router(AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        mods,
        vision: Arc::new(MockVisionClassifier {
            taxon_id: TaxonId::INaturalist(26_777),
            scientific_name: "Ambystoma mexicanum",
        }),
        taxonomy_endpoints: TaxonomyEndpoints::default(),
        inaturalist_api_token: None,
        max_moire_score: 0.35,
        enable_live_range_fallback: false,
    });

    // 1. Fresh user sees all 92 core Almanac entries with 0 discovered (isolated from live sync rows).
    let list_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac?user_id={test_user}&include_naturalista=false"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);
    let list_bytes = axum::body::to_bytes(list_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: AlmanacListResponse = serde_json::from_slice(&list_bytes).unwrap();
    assert_eq!(list.total_species, 92);
    assert_eq!(list.discovered_count, 0);
    assert_eq!(list.entries.len(), 92);

    // 2. Category + text search filter works cleanly across both core & new clades.
    let filtered_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac?category=reptiles_amphibians&q=ajolote&user_id={test_user}&include_naturalista=false"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(filtered_resp.status(), StatusCode::OK);
    let filtered_bytes = axum::body::to_bytes(filtered_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let filtered: AlmanacListResponse = serde_json::from_slice(&filtered_bytes).unwrap();
    assert_eq!(filtered.total_species, 92);
    assert_eq!(filtered.entries.len(), 1);
    assert_eq!(filtered.entries[0].scientific_name, "Ambystoma mexicanum");
    assert!(!filtered.entries[0].discovered);

    let sharks_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac?category=sharks&user_id={test_user}&include_naturalista=false"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sharks_resp.status(), StatusCode::OK);
    let sharks_bytes = axum::body::to_bytes(sharks_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let sharks: AlmanacListResponse = serde_json::from_slice(&sharks_bytes).unwrap();
    assert_eq!(sharks.entries.len(), 8);

    // 3. Record a verified wild capture for this user and confirm discovery badge flips to true.
    sqlx::query(
        r#"
        INSERT INTO captures (
            capture_id, user_id, device_public_key, image_sha256, captured_at_utc,
            location, zone_classification, taxon_source, taxon_external_id, vision_confidence
        )
        VALUES (
            $1, $2, $3, $4, NOW(),
            ST_SetSRID(ST_MakePoint(-99.10, 19.26), 4326)::geography, 'wild',
            'i_naturalist', 26777, 0.99
        )
        "#,
    )
    .bind(format!("almanac-cap-{}", Uuid::new_v4()))
    .bind(&test_user)
    .bind(vec![1_u8, 2, 3])
    .bind(vec![0xAA_u8; 32])
    .execute(&pool)
    .await
    .unwrap();

    let detail_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac/i_naturalist/26777?user_id={test_user}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail_resp.status(), StatusCode::OK);
    let detail_bytes = axum::body::to_bytes(detail_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let detail: AlmanacItemDto = serde_json::from_slice(&detail_bytes).unwrap();
    assert!(detail.discovered);
    assert_eq!(detail.best_variant.as_deref(), Some("wild"));
    assert_eq!(detail.capture_count, 1);

    // 4. Unknown taxon returns 404 Not Found.
    let not_found_resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/almanac/i_naturalist/999999999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(not_found_resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn subspecies_rollup_enriches_new_species_and_respects_geozones() {
    use axum::routing::get;
    use sqlx::Row;

    let pool = setup_test_db().await;
    let mods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let mods = Arc::new(load_mods_from_dir(&mods_dir).unwrap());

    // Ensure Panthera pardus (iNat 41963, not in base-pack) starts uncached so we test full offline enrichment.
    sqlx::query("DELETE FROM species_catalog WHERE taxon_source = 'i_naturalist' AND taxon_external_id = 41963")
        .execute(&pool)
        .await
        .unwrap();

    // Seed an OSM nature reserve + overlapping sanctuary in geo_zones at isolated synthetic coords (3.0..3.5).
    sqlx::query(
        r#"
        INSERT INTO geo_zones (osm_type, osm_id, name, zone_category, zone_type, boundary, region_tag, verified)
        VALUES
            (
                'relation',
                990001,
                'Reserva Cofre de Prueba',
                'nature_reserve',
                'nature_reserve',
                ST_Multi(ST_GeomFromText('POLYGON((3.00 3.00, 3.50 3.00, 3.50 3.50, 3.00 3.50, 3.00 3.00))', 4326)),
                'test-synthetic',
                TRUE
            ),
            (
                'way',
                990002,
                'Zoo de Prueba OSM',
                'sanctuary',
                'zoo',
                ST_Multi(ST_GeomFromText('POLYGON((3.10 3.10, 3.20 3.10, 3.20 3.20, 3.10 3.20, 3.10 3.10))', 4326)),
                'test-synthetic',
                TRUE
            )
        ON CONFLICT (osm_type, osm_id) DO UPDATE SET boundary = EXCLUDED.boundary
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Local loopback HTTP mock for iNaturalist /taxa, GBIF /species/match + IUCN, and Wikidata.
    let mock_api = Router::new()
        .route(
            "/inat/taxa/41963",
            get(|| async {
                axum::Json(json!({
                    "results": [{
                        "id": 41963,
                        "name": "Panthera pardus",
                        "preferred_common_name": "Leopardo",
                        "iconic_taxon_name": "Mammalia"
                    }]
                }))
            }),
        )
        .route(
            "/gbif/species/match",
            get(|| async {
                axum::Json(json!({
                    "usageKey": 5219436,
                    "speciesKey": 5219436,
                    "canonicalName": "Panthera pardus",
                    "phylum": "Chordata",
                    "class": "Mammalia",
                    "order": "Carnivora",
                    "family": "Felidae",
                    "matchType": "EXACT"
                }))
            }),
        )
        .route(
            "/gbif/species/5219436/iucnRedListCategory",
            get(|| async {
                axum::Json(json!({
                    "code": "VU",
                    "category": "VULNERABLE"
                }))
            }),
        )
        .route(
            "/wikidata",
            get(|axum::extract::RawQuery(q): axum::extract::RawQuery| async move {
                if q.unwrap_or_default().contains("wbsearchentities") {
                    axum::Json(json!({ "search": [{ "id": "Q34706" }] }))
                } else {
                    axum::Json(json!({
                        "entities": {
                            "Q34706": {
                                "labels": { "es": { "value": "Leopardo" } },
                                "descriptions": { "es": { "value": "especie de mamífero félido" } },
                                "claims": {
                                    "P225": [{ "mainsnak": { "datavalue": { "value": "Panthera pardus" } } }],
                                    "P2067": [{ "mainsnak": { "datavalue": { "value": { "amount": "+65", "unit": "http://www.wikidata.org/entity/Q11570" } } } }]
                                }
                            }
                        }
                    }))
                }
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, mock_api).await;
    });

    let endpoints = TaxonomyEndpoints {
        inaturalist_base_url: format!("http://{mock_addr}/inat"),
        gbif_base_url: format!("http://{mock_addr}/gbif"),
        wikidata_api_url: format!("http://{mock_addr}/wikidata"),
    };

    let mock_leopard_cv = r#"{
        "results": [{
            "combined_score": 93.5,
            "taxon": {
                "id": 991234,
                "name": "Panthera pardus orientalis",
                "rank": "subspecies",
                "rank_level": 5,
                "iconic_taxon_name": "Mammalia",
                "ancestors": [
                    { "id": 41962, "name": "Panthera", "rank": "genus", "rank_level": 20 },
                    { "id": 41963, "name": "Panthera pardus", "rank": "species", "rank_level": 10, "preferred_common_name": "Leopardo" }
                ]
            }
        }]
    }"#;

    let app = router(AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        mods: mods.clone(),
        vision: Arc::new(MockVisionClient::from_cached_json(
            mock_leopard_cv,
            None,
            0.35,
        )),
        taxonomy_endpoints: endpoints.clone(),
        inaturalist_api_token: None,
        max_moire_score: 0.35,
        enable_live_range_fallback: false,
    });

    // 1. Inside nature_reserve outside zoo -> mints Wild Panthera pardus (VU -> Rare), records subspecies_label.
    let reserve_cid = format!("test-rollup-reserve-{}", Uuid::new_v4());
    let body_reserve = signed_capture_body(
        &reserve_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 3.40,
            longitude: 3.40,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(body_reserve))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let minted: CaptureUploadResponse = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(minted.identification.scientific_name, "Panthera pardus");
    assert_eq!(
        minted.identification.subspecies_label.as_deref(),
        Some("Panthera pardus orientalis")
    );
    let card = minted.card.unwrap();
    assert_eq!(card.scientific_name, "Panthera pardus");
    assert_eq!(card.variant, CaptureVariant::Wild);
    assert_eq!(card.rarity, biodex_core::taxonomy::Rarity::Rare);

    // Verify species_catalog cached it with is_core_almanac = FALSE, category = 'felines', and captures stored subspecies_label + geo_zone_id.
    let catalog_row = sqlx::query(
        "SELECT average_mass_grams, iucn_status, category, is_core_almanac FROM species_catalog WHERE taxon_source = 'i_naturalist' AND taxon_external_id = 41963",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        catalog_row.try_get::<i64, _>("average_mass_grams").unwrap(),
        65_000
    );
    assert_eq!(
        catalog_row.try_get::<String, _>("iucn_status").unwrap(),
        "VU"
    );
    assert_eq!(
        catalog_row.try_get::<String, _>("category").unwrap(),
        "felines"
    );
    assert!(!catalog_row.try_get::<bool, _>("is_core_almanac").unwrap());

    let cap_row =
        sqlx::query("SELECT subspecies_label, geo_zone_id FROM captures WHERE capture_id = $1")
            .bind(&reserve_cid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        cap_row
            .try_get::<Option<String>, _>("subspecies_label")
            .unwrap()
            .as_deref(),
        Some("Panthera pardus orientalis")
    );
    assert!(
        cap_row
            .try_get::<Option<Uuid>, _>("geo_zone_id")
            .unwrap()
            .is_some()
    );

    // 2. Inside overlapping zoo in geo_zones -> Sanctuary takes priority over nature_reserve.
    let zoo_cid = format!("test-rollup-zoo-{}", Uuid::new_v4());
    let body_zoo = signed_capture_body(
        &zoo_cid,
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 3.15,
            longitude: 3.15,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let zoo_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(body_zoo))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(zoo_resp.status(), StatusCode::CREATED);
    let zoo_bytes = axum::body::to_bytes(zoo_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let zoo_minted: CaptureUploadResponse = serde_json::from_slice(&zoo_bytes).unwrap();
    assert_eq!(zoo_minted.card.unwrap().variant, CaptureVariant::Sanctuary);

    // 3. Broad taxonomic rank (genus) returns 422 taxonomic_rank_too_broad.
    let broad_app = router(AppState {
        db: pool,
        http: reqwest::Client::new(),
        mods,
        vision: Arc::new(MockVisionClient::from_cached_json(
            r#"{"results":[{"combined_score":91.0,"taxon":{"id":41962,"name":"Panthera","rank":"genus","rank_level":20}}]}"#,
            None,
            0.35,
        )),
        taxonomy_endpoints: endpoints,
        inaturalist_api_token: None,
        max_moire_score: 0.35,
        enable_live_range_fallback: false,
    });
    let broad_body = signed_capture_body(
        &format!("test-broad-{}", Uuid::new_v4()),
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 3.40,
            longitude: 3.40,
            accuracy_meters: 5.0,
        },
        false,
        false,
    );
    let broad_resp = broad_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(broad_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(broad_resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let broad_text = String::from_utf8(
        axum::body::to_bytes(broad_resp.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(broad_text.contains("taxonomic_rank_too_broad"));
}

#[tokio::test]
async fn dev_mode_unlocks_collection_and_naturalista_endpoints_work() {
    use axum::routing::get;

    let pool = setup_test_db().await;
    let mods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let mods = Arc::new(load_mods_from_dir(&mods_dir).unwrap());
    sync_almanac_catalog(&pool, &mods).await.unwrap();

    let fresh_user = format!("dev-preview-{}", Uuid::new_v4());

    let mock_api = Router::new()
        .route(
            "/inat/taxa",
            get(|| async {
                axum::Json(json!({
                    "results": [{
                        "id": 26777,
                        "name": "Ambystoma mexicanum",
                        "preferred_common_name": "Ajolote de Xochimilco",
                        "iconic_taxon_name": "Amphibia",
                        "observations_count": 3064,
                        "conservation_status": { "status": "CR" },
                        "default_photo": {
                            "medium_url": "https://example.org/axolotl-med.jpg",
                            "attribution": "(c) Ejemplo"
                        },
                        "wikipedia_url": "https://es.wikipedia.org/wiki/Ambystoma_mexicanum"
                    }]
                }))
            }),
        )
        .route(
            "/inat/observations/species_counts",
            get(|| async {
                axum::Json(json!({
                    "results": [{
                        "count": 42,
                        "taxon": {
                            "id": 35342,
                            "name": "Iguana iguana",
                            "preferred_common_name": "Iguana Verde",
                            "iconic_taxon_name": "Reptilia",
                            "observations_count": 90000
                        }
                    }]
                }))
            }),
        )
        .route(
            "/inat/observations",
            get(
                |axum::extract::RawQuery(q): axum::extract::RawQuery| async move {
                    if q.unwrap_or_default().contains("taxon_id=35342") {
                        axum::Json(json!({ "total_results": 924, "results": [] }))
                    } else {
                        axum::Json(json!({ "total_results": 0, "results": [] }))
                    }
                },
            ),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, mock_api).await;
    });

    let app = router(AppState {
        db: pool,
        http: reqwest::Client::new(),
        mods,
        vision: Arc::new(MockVisionClassifier {
            taxon_id: TaxonId::INaturalist(35_342),
            scientific_name: "Iguana iguana",
        }),
        taxonomy_endpoints: TaxonomyEndpoints {
            inaturalist_base_url: format!("http://{mock_addr}/inat"),
            gbif_base_url: format!("http://{mock_addr}/gbif"),
            wikidata_api_url: format!("http://{mock_addr}/wikidata"),
        },
        inaturalist_api_token: None,
        max_moire_score: 0.35,
        enable_live_range_fallback: true,
    });

    // 1. Dev Mode unlocks every species for a fresh zero-capture user.
    let dev_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac?user_id={fresh_user}&dev_mode=true"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dev_resp.status(), StatusCode::OK);
    let dev_bytes = axum::body::to_bytes(dev_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let dev_list: AlmanacListResponse = serde_json::from_slice(&dev_bytes).unwrap();
    assert!(dev_list.total_species >= 92);
    assert_eq!(dev_list.discovered_count, dev_list.total_species);
    assert!(dev_list.entries.iter().all(|e| {
        e.discovered && e.capture_count >= 1 && e.naturalista_url.starts_with("https://")
    }));

    // 2. NaturaLista search & nearby endpoints return structured fauna results.
    let search_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac/naturalista/search?q=ajolote&user_id={fresh_user}"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search_resp.status(), StatusCode::OK);
    let search_bytes = axum::body::to_bytes(search_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let search: NaturaListaDiscoveryResponse = serde_json::from_slice(&search_bytes).unwrap();
    assert_eq!(search.entries.len(), 1);
    assert!(search.entries[0].in_catalog);
    assert_eq!(
        search.entries[0].naturalista_url,
        "https://www.naturalista.mx/taxa/26777"
    );

    let nearby_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/almanac/naturalista/nearby?lat=19.19&lng=-96.14&radius_km=20&user_id={fresh_user}&dev_mode=true"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(nearby_resp.status(), StatusCode::OK);
    let nearby_bytes = axum::body::to_bytes(nearby_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let nearby: NaturaListaDiscoveryResponse = serde_json::from_slice(&nearby_bytes).unwrap();
    assert_eq!(nearby.entries.len(), 1);
    assert_eq!(nearby.entries[0].local_observations_count, Some(42));
    assert!(nearby.entries[0].discovered);

    // 3. Regional NaturaLista presence fallback mints Wild for Iguana iguana outside explicit polygons.
    let cap_body = signed_capture_body(
        &format!("test-naturalista-wild-{}", Uuid::new_v4()),
        CaptureSource::CameraXLive,
        GeoPoint {
            latitude: 19.1883,
            longitude: -96.1463,
            accuracy_meters: 8.0,
        },
        false,
        false,
    );
    let cap_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/captures")
                .header("content-type", "application/json")
                .body(Body::from(cap_body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cap_resp.status(), StatusCode::CREATED);
    let cap_bytes = axum::body::to_bytes(cap_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let minted: CaptureUploadResponse = serde_json::from_slice(&cap_bytes).unwrap();
    assert_eq!(minted.status, "minted");
    assert_eq!(
        minted.naturalista_url.as_deref(),
        Some("https://www.naturalista.mx/taxa/35342")
    );
}

#[tokio::test]
async fn naturalista_clade_sync_filters_subspecies_and_supports_pagination() {
    use axum::http::HeaderMap;
    use axum::routing::get;
    use sqlx::Row;

    let pool = setup_test_db().await;
    let mods_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods");
    let mods = Arc::new(load_mods_from_dir(&mods_dir).unwrap());
    sync_almanac_catalog(&pool, &mods).await.unwrap();

    // Ensure synthetic test taxon IDs do not linger across test runs.
    sqlx::query(
        "DELETE FROM species_catalog WHERE taxon_source = 'i_naturalist' AND taxon_external_id BETWEEN 9000001 AND 9000010",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mock_api = Router::new().route(
        "/inat/taxa",
        get(
            |headers: HeaderMap, axum::extract::RawQuery(raw_q): axum::extract::RawQuery| async move {
                assert_eq!(
                    headers.get("authorization").and_then(|v| v.to_str().ok()),
                    Some("Bearer test-inat-token")
                );
                let q = raw_q.unwrap_or_default();
                if q.contains("taxon_id=42043") {
                    axum::Json(json!({
                        "results": [
                            {
                                "id": 9_000_001,
                                "name": "Vulpes velox",
                                "preferred_common_name": "Zorro Veloz Prueba",
                                "rank": "species",
                                "rank_level": 10,
                                "is_active": true,
                                "extinct": false,
                                "ancestor_ids": [42043],
                                "observations_count": 2100,
                                "default_photo": { "medium_url": "https://example.org/vulpes.jpg" }
                            },
                            {
                                "id": 9_000_002,
                                "name": "Canis lupus baileyi",
                                "preferred_common_name": "Lobo mexicano (subespecie)",
                                "rank": "subspecies",
                                "rank_level": 5,
                                "is_active": true,
                                "extinct": false,
                                "ancestor_ids": [42043],
                                "default_photo": { "medium_url": "https://example.org/subsp.jpg" }
                            },
                            {
                                "id": 9_000_003,
                                "name": "Aenocyon dirus",
                                "rank": "species",
                                "rank_level": 10,
                                "is_active": true,
                                "extinct": true,
                                "ancestor_ids": [42043],
                                "default_photo": { "medium_url": "https://example.org/fossil.jpg" }
                            },
                            {
                                "id": 9_000_004,
                                "name": "Canis × latrans",
                                "rank": "species",
                                "rank_level": 10,
                                "is_active": true,
                                "extinct": false,
                                "ancestor_ids": [42043],
                                "default_photo": { "medium_url": "https://example.org/hybrid.jpg" }
                            }
                        ]
                    }))
                } else if q.contains("taxon_id=47273") {
                    axum::Json(json!({
                        "results": [{
                            "id": 9_000_005,
                            "name": "Carcharias taurus",
                            "preferred_common_name": "Tiburón Toro Arena Prueba",
                            "rank": "species",
                            "rank_level": 10,
                            "is_active": true,
                            "extinct": false,
                            "ancestor_ids": [47273],
                            "observations_count": 5000,
                            "default_photo": { "medium_url": "https://example.org/shark.jpg" }
                        }]
                    }))
                } else if q.contains("taxon_id=43367") {
                    axum::Json(json!({
                        "results": [{
                            "id": 43_584,
                            "name": "Homo sapiens",
                            "rank": "species",
                            "rank_level": 10,
                            "is_active": true,
                            "extinct": false,
                            "ancestor_ids": [43367],
                            "default_photo": { "medium_url": "https://example.org/human.jpg" }
                        }]
                    }))
                } else if q.contains("taxon_id=39392") {
                    axum::Json(json!({
                        "results": [{
                            "id": 9_000_006,
                            "name": "Varanus komodoensis",
                            "preferred_common_name": "Dragón de Komodo Prueba",
                            "rank": "species",
                            "rank_level": 10,
                            "is_active": true,
                            "extinct": false,
                            "ancestor_ids": [26036, 39392],
                            "observations_count": 2913,
                            "default_photo": { "medium_url": "https://example.org/komodo.jpg" }
                        }]
                    }))
                } else {
                    axum::Json(json!({ "results": [] }))
                }
            },
        ),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, mock_api).await;
    });

    let app = router(AppState {
        db: pool.clone(),
        http: reqwest::Client::new(),
        mods,
        vision: Arc::new(MockVisionClassifier {
            taxon_id: TaxonId::INaturalist(26_777),
            scientific_name: "Ambystoma mexicanum",
        }),
        taxonomy_endpoints: TaxonomyEndpoints {
            inaturalist_base_url: format!("http://{mock_addr}/inat"),
            gbif_base_url: format!("http://{mock_addr}/gbif"),
            wikidata_api_url: format!("http://{mock_addr}/wikidata"),
        },
        inaturalist_api_token: Some("test-inat-token".to_string()),
        max_moire_score: 0.35,
        enable_live_range_fallback: false,
    });

    // 1. Sync canids: keeps 1 living species (9000001) and filters subspecies/extinct/hybrid (3).
    let canid_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/almanac/naturalista/sync?page=1&category=canids")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(canid_resp.status(), StatusCode::OK);
    let canid_bytes = axum::body::to_bytes(canid_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let canid_report: NaturaListaSyncReport = serde_json::from_slice(&canid_bytes).unwrap();
    assert_eq!(canid_report.fetched_candidates, 4);
    assert_eq!(canid_report.filtered_non_species, 3);
    assert_eq!(canid_report.upserted_species, 1);

    // 2. Sync sharks: upserts 1 shark (9000005).
    let shark_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/almanac/naturalista/sync?page=1&category=sharks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(shark_resp.status(), StatusCode::OK);
    let shark_bytes = axum::body::to_bytes(shark_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let shark_report: NaturaListaSyncReport = serde_json::from_slice(&shark_bytes).unwrap();
    assert_eq!(shark_report.upserted_species, 1);

    // 3. Sync primates: Homo sapiens (43584) is filtered out.
    let primate_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/almanac/naturalista/sync?page=1&category=primates")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(primate_resp.status(), StatusCode::OK);
    let primate_bytes = axum::body::to_bytes(primate_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let primate_report: NaturaListaSyncReport = serde_json::from_slice(&primate_bytes).unwrap();
    assert_eq!(primate_report.upserted_species, 0);
    assert_eq!(primate_report.filtered_non_species, 1);

    // 4. Verify database rows and exclusion of subspecies/extinct/hybrid/human.
    let db_rows = sqlx::query(
        "SELECT taxon_external_id, taxonomic_rank, is_naturalista_synced, category FROM species_catalog WHERE taxon_source = 'i_naturalist' AND taxon_external_id IN (43584, 9000001, 9000002, 9000003, 9000004, 9000005) ORDER BY taxon_external_id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(db_rows.len(), 2);
    assert_eq!(
        db_rows[0].try_get::<i64, _>("taxon_external_id").unwrap(),
        9_000_001
    );
    assert_eq!(
        db_rows[0].try_get::<String, _>("taxonomic_rank").unwrap(),
        "species"
    );
    assert!(
        db_rows[0]
            .try_get::<bool, _>("is_naturalista_synced")
            .unwrap()
    );
    assert_eq!(
        db_rows[0].try_get::<String, _>("category").unwrap(),
        "canids"
    );
    assert_eq!(
        db_rows[1].try_get::<i64, _>("taxon_external_id").unwrap(),
        9_000_005
    );
    assert_eq!(
        db_rows[1].try_get::<String, _>("category").unwrap(),
        "sharks"
    );

    // 5. Verify pagination, category_counts, and captured_filter on GET /api/v1/almanac.
    let page_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/almanac?dev_mode=true&limit=5&offset=0")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(page_resp.status(), StatusCode::OK);
    let page_bytes = axum::body::to_bytes(page_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let page_list: AlmanacListResponse = serde_json::from_slice(&page_bytes).unwrap();
    assert_eq!(page_list.entries.len(), 5);
    assert_eq!(page_list.matched_count, page_list.total_species);
    assert!(
        page_list
            .category_counts
            .get("canids")
            .copied()
            .unwrap_or(0)
            >= 7
    );
    assert!(
        page_list
            .category_counts
            .get("sharks")
            .copied()
            .unwrap_or(0)
            >= 9
    );

    let missing_in_dev_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/almanac?dev_mode=true&captured_filter=missing")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_in_dev_resp.status(), StatusCode::OK);
    let missing_bytes = axum::body::to_bytes(missing_in_dev_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let missing_list: AlmanacListResponse = serde_json::from_slice(&missing_bytes).unwrap();
    assert_eq!(missing_list.matched_count, 0);
    assert!(missing_list.entries.is_empty());

    // 6. Sync with backend_only=true keeps UI list lightweight while making taxon searchable & backend-indexed.
    let backend_only_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/almanac/naturalista/sync?page=2&category=reptiles_amphibians&backend_only=true")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(backend_only_resp.status(), StatusCode::OK);
    let backend_bytes = axum::body::to_bytes(backend_only_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let backend_report: NaturaListaSyncReport = serde_json::from_slice(&backend_bytes).unwrap();
    assert_eq!(backend_report.upserted_species, 1);
    assert!(backend_report.total_backend_indexed > backend_report.total_catalog_species);

    let search_komodo_resp = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/almanac?dev_mode=true&q=Komodo%20Prueba")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(search_komodo_resp.status(), StatusCode::OK);
    let komodo_bytes = axum::body::to_bytes(search_komodo_resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let komodo_list: AlmanacListResponse = serde_json::from_slice(&komodo_bytes).unwrap();
    assert_eq!(komodo_list.entries.len(), 1);
    assert_eq!(
        komodo_list.entries[0].taxon_id,
        TaxonId::INaturalist(9_000_006)
    );
    assert!(komodo_list.backend_indexed_count > komodo_list.total_species);

    // Cleanup synthetic records so local dev Postgres remains populated strictly with real species.
    sqlx::query(
        "DELETE FROM species_catalog WHERE taxon_source = 'i_naturalist' AND taxon_external_id IN (9000001, 9000005, 9000006)",
    )
    .execute(&pool)
    .await
    .unwrap();
}
