# ARCHITECTURE.md — Proyecto BioDex (Nombre Provisional)

## 1. Visión y Propósito

Plataforma Open Source (licencia MIT / Apache-2.0) que fusiona ciencia ciudadana, ecoturismo y juego de cartas/combate por turnos. Convierte el avistamiento real de fauna en criaturas coleccionables verificables, incentivando la exploración de reservas naturales y zoológicos sin depender de modelos "Pay-to-Win".

## 2. Principios de Diseño

1. **Data-Driven & Procedural First:** Es inviable hardcodear millones de especies. Las estadísticas, tipos y ataques se calculan algorítmicamente a partir de taxonomía y rasgos biológicos reales (GBIF, Wikidata, Lista Roja UICN).
2. **Extensibilidad Comunitaria (Mod-Friendly):** Cualquier biólogo, ecólogo o desarrollador puede añadir nuevas especies, corregir ataques, crear "Expediciones de Bioma" (ej. *Cascadas de Veracruz*, *Selva Lacandona*) o diseñar reglas personalizadas mediante packs declarativos (`TOML`/`JSON`) y plugins WASM.
3. **Integridad Verificable (Anti-Spoofing):** Las capturas competitivas exigen prueba de vida en cámara, firma de telemetría de hardware y validación geoespacial PostGIS.

## 3. Stack Tecnológico y Estructura del Monorepo

| Capa | Ubicación | Stack |
| :--- | :--- | :--- |
| Motor Core | `core-engine/` | Rust puro (lógica determinista, stats procedurales, mods, criptografía) |
| Backend API | `backend/` | Rust (`Axum` + `Tokio` + `SQLx`) + `PostgreSQL` + `PostGIS` |
| Cliente Móvil | `mobile-android/` | Android nativo en Kotlin + Jetpack Compose + CameraX |
| Extensibilidad | `mods/` | Packs declarativos (`TOML`/`JSON`) + soporte futuro WASM (`Extism`/`Wasmtime`, Fase 3) |
| Documentación | `docs/` | Contratos de datos aprobados, ADRs y guías de contribución |

```text
biodex/
├── ARCHITECTURE.md          # Este documento (fuente de verdad arquitectónica)
├── AGENTS.md                # Reglas de estilo, comandos de build y flujo para agentes IA
├── core-engine/             # Crate en Rust: motor de combate, stats procedurales y loader de mods
│   └── src/
│       ├── taxonomy/        # [Fase 1] Mapeo de rasgos biológicos -> tipos, rareza y stats base
│       ├── mods/            # [Fase 1] Parser y validador de packs comunitarios (TOML/JSON)
│       ├── crypto/          # [Fase 1] Verificación de firmas y metadatos de captura
│       └── combat/          # [Fase 2] Motor de batalla por turnos determinista
├── backend/                 # Crate en Rust (Axum): API REST/gRPC, PostGIS, auth e intercambios
│   ├── migrations/          # [Fase 1] Esquemas SQL + extensiones PostGIS
│   └── src/
│       ├── geo/             # [Fase 1] Validación de polígonos (Salvaje vs. Santuario)
│       ├── vision/          # [Fase 1] Orquestación con modelo/API taxonómica (BioCLIP / iNaturalist / GBIF)
│       └── market/          # [Fase 2] Sistema de intercambio P2P entre jugadores
├── mobile-android/          # [Fase 1] App nativa en Kotlin + Jetpack Compose + CameraX
├── mods/                    # Repositorio de packs comunitarios oficiales y de terceros
│   └── base-pack/           # Pack base oficial (nombre provisional en Fase 1)
└── docs/                    # Documentación técnica, contratos de datos y ADRs
```

## 4. Mecánicas Nucleares

### A. Captura y Validación Anti-Trampas

- **Bloqueo de Galería:** Solo se permiten capturas directas desde el pipeline CameraX de la app (las fotos de galería solo sirven para modo enciclopedia sin valor competitivo ni de intercambio).
- **Prueba de Entorno:** La app empaqueta en un payload firmado: imagen RAW/JPEG comprimida + timestamp + coordenadas GPS + lectura de acelerómetro/giroscopio durante el disparo + heurística ligera anti-Moiré (detección de píxeles de pantalla).
- **Clasificación Geoespacial (PostGIS):**
  - **Variante SALVAJE:** El GPS coincide con el rango de distribución natural de la especie (ej. cascadas, bosques, costas, parques nacionales). Otorga el 100% del potencial genético (IVs máximos) e insignia de explorador.
  - **Variante SANTUARIO:** El GPS cae dentro del polígono registrado de un zoológico, acuario o UMA. Es 100% válida para colección, cría/sinergias e intercambio, con un multiplicador estadístico ligeramente menor para premiar el turismo de campo.
  - **Descarte:** Especie fuera de rango natural y fuera de cualquier polígono de santuario registrado.

### B. Generación Procedural de Cartas (Motor Taxonómico)

- **Identificación:** El pipeline de visión devuelve el `taxon_id` (GBIF / iNaturalist) y nivel de confianza.
- **Extracción de Atributos:** Se consultan metadatos biológicos: Filo/Clase, masa corporal promedio, dieta (carnívoro, herbívoro, omnívoro, detritívoro), hábitat (abisal, terrestre, aéreo, dulceacuícola), defensas (veneno, camuflaje, caparazón) y estado UICN.
- **Asignación de Rareza (Lista Roja UICN):**
  - `LC` (Preocupación Menor) → Común
  - `NT` (Casi Amenazado) → Poco Común
  - `VU` (Vulnerable) → Rara
  - `EN` (En Peligro) → Épica
  - `CR` (En Peligro Crítico) → Mítica / Legendaria
- **Balanceo Competitivo (Sistema de Energía):**
  - Cada carta tiene un **Coste de Despliegue (1 a 10)** proporcional a su masa/poder bruto. Un calamar gigante o un oso polar tienen estadísticas masivas pero coste 8–9; una mantis religiosa o una rana dardo tienen coste 2–3 y habilitan combos rápidos de veneno o emboscada.

## 5. Arquitectura de Mods y Plugins Comunitarios

Para escalar sin cuello de botella del equipo central, el motor lee capas en orden de prioridad:

```text
[Reglas Procedurales Base] -> [Pack Oficial base-pack] -> [Mods Comunitarios Instalados]
```

### Formato Declarativo de Mod (`mod.toml` + `species/*.toml`)

Los colaboradores pueden enviar Pull Requests o distribuir carpetas con archivos `.toml` (y `.json` donde aplique) que definan:

- **Overrides de Especie:** Asignar ataques únicos o artes/sprites alternativos a especies específicas (ej. `Architeuthis dux` → ataque exclusivo *"Abrazo Abisal"*).
- **Expediciones / Misiones de Turismo:** Definir un polígono GeoJSON (ej. un parque natural local) y una lista de 10 especies objetivo que desbloquean medallas o cartas variantes al completarse.
- **Hooks Lógicos (Fase 3):** Soporte para módulos `.wasm` aislados (`Extism` / `Wasmtime`) que definan nuevos estados alterados o efectos climáticos en combate.

## 6. Roadmap de Implementación

### Fase 1 — MVP "BioDex Core" (Actual)

- [x] Definir structs base en `core-engine` (`SpeciesCard`, `BiologicalTraits`, `CaptureMetadata`, `ModManifest`, `AlmanacEntry`).
- [x] Implementar el generador procedural de estadísticas, verificación ECDSA P-256 y parser de packs `.toml` (overrides + Almanaque).
- [x] Levantar backend en Rust (`Axum` + `Tokio` + `SQLx`) + PostgreSQL/PostGIS con validación de polígonos (prioridad `Santuario` sobre `Salvaje`) y endpoints del Almanaque (`/api/v1/almanac`).
- [x] Prototipo Android (CameraX en vivo limpio sin galería ni especies de prueba + firma Keystore P-256 + pestaña Almanaque de 24 especies básicas con insignia de capturado).
- [x] Pipeline real de visión taxonómica (iNaturalist CV + normalización `subspecies -> species` + rechazo `taxonomic_rank_too_broad`), enriquecimiento automático (GBIF + Wikidata), modo `mock` offline e ingesta geoespacial desde OSM Overpass (`sync-geozones` → `geo_zones`).

### Fase 2 — Combate 1v1 e Intercambios

- Motor determinista de batalla por turnos (combate 1v1 con equipos 3v3 y gestión de energía).
- Sistema de intercambio P2P con historial de procedencia de captura (ubicación y fecha original).

### Fase 3 — Ecosistema Open Source y Expediciones

- Registro abierto de polígonos de parques/santuarios validado por comunidad.
- Soporte completo para plugins WASM y torneos con reglas personalizadas por mod.
