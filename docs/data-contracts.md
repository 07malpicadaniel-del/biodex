# Contratos de Datos Aprobados — Fase 1 (MVP)

Este documento registra los contratos de datos aprobados por el Arquitecto de Software para la Fase 1 de **BioDex**.

## 1. Taxonomía y Cartas (`core-engine::taxonomy`)

### Decisiones Aprobadas
- **Unidades físicas:** La masa corporal promedio se representa en gramos enteros (`u64`) para garantizar aritmética 100% determinista sin punto flotante.
- **Multiplicadores por variante de captura:**
  - `Wild` (Salvaje): `100%` sobre los stats base.
  - `Sanctuary` (Santuario): `85%` sobre los stats base (aritmética entera).
- **Rareza por Lista Roja UICN:**
  - `LC` / `Unknown` → `Common`
  - `NT` → `Uncommon`
  - `VU` → `Rare`
  - `EN` → `Epic`
  - `CR` → `Mythic`
- **Coste de Despliegue:** Entero acotado `1..=10` derivado determinísticamente de la masa y potencia biológica.

### Estructuras Base
- `TaxonId`: `Gbif(u64)` | `INaturalist(u64)`
- `BiologicalTraits`: `taxon_id`, `scientific_name`, `common_name`, `phylum`, `taxonomic_class`, `average_mass_grams`, `diet`, `habitat`, `defenses` (`BTreeSet<DefenseTrait>`), `iucn_status`.
- `SpeciesCard`: `taxon_id`, `scientific_name`, `display_name`, `affinity`, `rarity`, `variant`, `deploy_cost`, `stats` (`CardStats`), `moves` (`Vec<CardMove>`), `art_override`, `applied_pack`.

---

## 2. Captura e Integridad Anti-Trampas (`core-engine::crypto`)

### Decisiones Aprobadas
- **Algoritmo de firma:** **ECDSA P-256** (compatible de forma nativa con Android Hardware Keystore `SHA256withECDSA`).
- **Formatos soportados en verificación:**
  - Clave pública: X.509 SPKI DER (estándar `PublicKey.encoded` en Android) o SEC1 raw.
  - Firma: ASN.1 DER (estándar `SHA256withECDSA`) o raw `(r || s)` de 64 bytes.
- **Bloqueo competitivo de galería:** `CaptureSource::Gallery` es rechazado explícitamente por el validador competitivo; solo `CaptureSource::CameraXLive` es válido.
- **Payload canónico (`BIODEX_CAPTURE_V1`):** Serialización binaria big-endian determinista sobre `capture_id`, `captured_at_utc_ms`, `source`, coordenadas GPS, telemetría IMU (`accel_xyz`, `gyro_xyz`), `moire_score` e `image_sha256`.

---

## 3. Mods Declarativos (`core-engine::mods`)

### Decisiones Aprobadas
- **Pack base oficial temporal:** `mods/base-pack/` (`ModLayer::OfficialBase`).
- **Orden de resolución de capas:** `[Reglas Procedurales Base] -> [base-pack] -> [Mods Comunitarios]` (ordenados por capa, prioridad ascendente y desempate alfabético por `id`).
- **Formato de archivos:**
  - `mod.toml`: `schema_version` (`1`), `id`, `name`, `version`, `authors`, `layer`, `priority`, `description`, `species_paths`, `almanac_paths`.
  - `species/*.toml`: `scientific_name`, `display_name_override`, `art_override`, `moves_override`.
  - `almanac/*.toml`: listado `[[entries]]` con `BiologicalTraits` aplanado + `category` (`canids | felines | primates | mammals | birds | reptiles_amphibians | sharks | invertebrates | insects_arachnids | aquatic`), `summary` y `region_hint` (92 especies curadas en `base-pack`).

---

## 4. Colección / Almanaque e Integración NaturaLista (`backend` + `mobile-android`)

- **Visibilidad y Login con Modo Dev:** El cliente muestra pantalla de Login al iniciar; cuando `dev_mode=true`, `GET /api/v1/almanac` desbloquea todas las especies catalogadas para visualizar la colección completa con fotografías y fichas biológicas (sin mostrar estadísticas ni movimientos de combate en la UI hasta Fase 2).
- **Persistencia:** Migraciones `0002_almanac_metadata.sql`, `0004_species_media_and_naturalista.sql` y `0005_naturalista_synced_catalog.sql` extienden `species_catalog` (`summary`, `category`, `region_hint`, `is_core_almanac`, `image_url`, `photo_attribution`, `wikipedia_url`, `naturalista_observations_count`, `taxonomic_rank = 'species'`, `is_naturalista_synced`), sincronizándose desde `ModRegistry` y la conexión `.env` a NaturaLista (`INATURALIST_TAXA_URL` + `INATURALIST_API_TOKEN`) con exclusión estricta de subespecies, híbridos, taxones extintos y flora/Homo sapiens.
- **Endpoints HTTP y CLI de Colección:**
  - `GET /api/v1/almanac?q=&category=&habitat=&user_id=&dev_mode=&captured_filter=&include_naturalista=&limit=&offset=` (retorna `matched_count` y `category_counts`)
  - `POST /api/v1/almanac/naturalista/sync?page=&category=` (sincroniza clados verificados desde NaturaLista en rango `species`)
  - `GET /api/v1/almanac/{source}/{taxon_id}?user_id=&dev_mode=`
  - `GET /api/v1/almanac/naturalista/search?q=&user_id=&dev_mode=`
  - `GET /api/v1/almanac/naturalista/nearby?lat=&lng=&radius_km=&user_id=&dev_mode=`
  - `GET /api/v1/almanac/naturalista/taxon/{id}?user_id=&dev_mode=`
  - Binario CLI: `cargo run --bin sync-naturalista -- --pages 1 [--category <clade>]`

---

## 5. Normalización a Especie Base (Species Rollup), Visión e Ingesta Overpass

- **Unidad atómica (`TaxonomicRank::Species`):** Todo `VisionIdentification` y registro en `species_catalog` se normaliza al rango `species` (binomial). Si iNaturalist detecta `subspecies`, `variety` o `form`, se resuelve el ancestro de rango `species` vía `/v1/taxa/{id}?locale=es` y se conserva `captures.subspecies_label` solo como dato informativo.
- **Umbral de rango mínimo:** Si la visión solo alcanza `genus`, `family` o rango superior, la API rechaza la captura con HTTP `422` (`taxonomic_rank_too_broad`).
- **Enriquecimiento automático:** Especies fuera del Almanaque base se enriquecen consultando iNaturalist + GBIF (`/species/match` + `/iucnRedListCategory`) + Wikidata (`P2067` masa adulta, `P141` UICN, descripción en español) y se cachean en `species_catalog` (`is_core_almanac = FALSE`).
- **Modo Mock offline:** `VISION_PROVIDER=mock` usa `MockVisionClient` con respuestas JSON cacheadas para pruebas deterministas sin red.
- **Ingesta geoespacial Overpass (`geo_zones`) y respaldo NaturaLista:** Migración `backend/migrations/0003_geozones_and_subspecies.sql` y binario `cargo run --bin sync-geozones` importan polígonos reales de zoológicos/acuarios (`sanctuary`) y reservas naturales (`Wild`). Si una especie `i_naturalist` carece de polígonos propios en `species_wild_ranges` y `ENABLE_NATURALISTA_RANGE_FALLBACK=true`, el backend verifica presencia regional verificable (`radius=150 km`) vía `/observations` para validar capturas de fauna local.
