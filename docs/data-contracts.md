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
  - `almanac/*.toml`: listado `[[entries]]` con `BiologicalTraits` aplanado + `category` (`birds | mammals | reptiles_amphibians | insects_arachnids | aquatic`), `summary` y `region_hint`.

---

## 4. Almanaque / Diccionario de Fauna Básica (`backend` + `mobile-android`)

- **Visibilidad aprobada:** Todas las especies básicas del Almanaque (24 iniciales en `mods/base-pack/almanac/`) muestran su ficha enciclopédica y estadísticas base desde el inicio, acompañadas de la insignia de estado **`CAPTURADO`** (con variante `SALVAJE` / `SANTUARIO`) o **`POR DESCUBRIR`**.
- **Persistencia:** Migración `backend/migrations/0002_almanac_metadata.sql` extiende `species_catalog` (`summary`, `category`, `region_hint`, `is_core_almanac`) y se sincroniza al arrancar el backend desde `ModRegistry`.
- **Endpoints HTTP:**
  - `GET /api/v1/almanac?q=&category=&habitat=&user_id=`
  - `GET /api/v1/almanac/{source}/{taxon_id}?user_id=`

---

## 5. Normalización a Especie Base (Species Rollup), Visión e Ingesta Overpass

- **Unidad atómica (`TaxonomicRank::Species`):** Todo `VisionIdentification` y registro en `species_catalog` se normaliza al rango `species` (binomial). Si iNaturalist detecta `subspecies`, `variety` o `form`, se resuelve el ancestro de rango `species` vía `/v1/taxa/{id}?locale=es` y se conserva `captures.subspecies_label` solo como dato informativo.
- **Umbral de rango mínimo:** Si la visión solo alcanza `genus`, `family` o rango superior, la API rechaza la captura con HTTP `422` (`taxonomic_rank_too_broad`).
- **Enriquecimiento automático:** Especies fuera del Almanaque base se enriquecen consultando iNaturalist + GBIF (`/species/match` + `/iucnRedListCategory`) + Wikidata (`P2067` masa adulta, `P141` UICN, descripción en español) y se cachean en `species_catalog` (`is_core_almanac = FALSE`).
- **Modo Mock offline:** `VISION_PROVIDER=mock` usa `MockVisionClient` con respuestas JSON cacheadas para pruebas deterministas sin red.
- **Ingesta geoespacial Overpass (`geo_zones`):** Migración `backend/migrations/0003_geozones_and_subspecies.sql` y binario `cargo run --bin sync-geozones` importan polígonos reales de zoológicos/acuarios (`sanctuary` → 85%) y reservas naturales (`nature_reserve` → `Wild` 100%) mediante `UPSERT` por `(osm_type, osm_id)`.
