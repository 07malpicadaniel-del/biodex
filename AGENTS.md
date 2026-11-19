# AGENTS.md — Convenciones, Flujo y Comandos de BioDex

Este archivo define las reglas operativas, convenciones de código y comandos de compilación/test para cualquier agente IA o colaborador humano que trabaje en el monorepo **BioDex**.

> **Lectura obligatoria previa:** Revisa [ARCHITECTURE.md](./ARCHITECTURE.md) antes de diseñar o implementar cualquier cambio.

---

## 1. Modelo de Colaboración y Flujo de Trabajo

1. **Roles:**
   - **Arquitecto de Software / Product Owner (Usuario):** Define la arquitectura, los contratos de datos, los esquemas SQL y las prioridades de cada fase.
   - **Ingeniero de Software Principal / Implementador (Agente IA):** Propone diseños técnicos detallados, escribe el código de producción y sus pruebas, y mantiene la coherencia del repositorio.
2. **Design-Before-Code (Obligatorio):**
   - Antes de implementar cualquier módulo complejo, endpoint o tabla SQL, presenta al Arquitecto el diseño de `struct`/`enum`/`trait`, firmas públicas y/o esquema SQL para su aprobación explícita.
3. **Foco estricto en la Fase Activa (Cero sobre-ingeniería temprana):**
   - El proyecto se encuentra en **Fase 1 — MVP "BioDex Core"**.
   - **No implementar** en esta fase mecánicas de **Fase 2** (`core-engine/src/combat/` 3v3 completo, `backend/src/market/` P2P) ni de **Fase 3** (plugins WASM vía `Extism`/`Wasmtime`, registro comunitario abierto).
   - Mantén las carpetas futuras reservadas sin añadir código especulativo ni dependencias no usadas.

---

## 2. Estructura del Monorepo y Fronteras de Arquitectura

```text
biodex/
├── ARCHITECTURE.md          # Fuente de verdad arquitectónica y roadmap
├── AGENTS.md                # Este archivo (reglas, estilo y comandos)
├── core-engine/             # Crate Rust puro (sin red, sin DB, sin async runtime)
│   └── src/
│       ├── taxonomy/        # [Fase 1] Rasgos biológicos -> tipo, rareza UICN, coste y stats
│       ├── mods/            # [Fase 1] Parser y validador de packs declarativos (TOML/JSON)
│       ├── crypto/          # [Fase 1] Verificación criptográfica de payloads de captura
│       └── combat/          # [Fase 2] Motor determinista de batalla por turnos
├── backend/                 # Crate Rust (Axum + Tokio + SQLx + PostgreSQL/PostGIS)
│   ├── migrations/          # [Fase 1] Migraciones SQL incrementales (PostGIS SRID 4326)
│   └── src/
│       ├── geo/             # [Fase 1] Validación geoespacial (Salvaje vs. Santuario vs. Descarte)
│       ├── vision/          # [Fase 1] Orquestación taxonómica (BioCLIP / iNaturalist / GBIF)
│       └── market/          # [Fase 2] Intercambio P2P entre jugadores
├── mobile-android/          # [Fase 1] Kotlin + Jetpack Compose + CameraX
├── mods/                    # Packs de especies, reglas y expediciones
│   └── base-pack/           # Pack oficial base (nombre provisional en Fase 1)
└── docs/                    # Contratos de datos aprobados, ADRs y guías de contribución
```

### Reglas de Dependencia entre Capas

- `core-engine` **nunca** debe depender de `backend`, `tokio`, `axum`, `sqlx` ni realizar I/O de red/base de datos. Debe compilar de forma aislada y determinista.
- `backend` consume `core-engine` como librería local del workspace de Cargo.
- `mobile-android` se comunica con `backend` únicamente a través de contratos HTTP/JSON versionados (y en el futuro puede reutilizar reglas de `core-engine` si se requiere, sin duplicar fórmulas de balance).
- `mods/` contiene datos declarativos (`TOML`/`JSON`) validados por `core-engine::mods`.

---

## 3. Convenciones de Código

### A. Rust (`core-engine/` y `backend/`)

- **Estilo y Linting:**
  - Código formateado con `rustfmt` (`cargo fmt`).
  - Cero advertencias permitidas en `clippy` (`-D warnings`).
- **Manejo de Errores:**
  - Prohibido usar `.unwrap()`, `.expect()` o `panic!()` en código de producción (permitidos únicamente dentro de `#[cfg(test)]`).
  - En `core-engine`, usar tipos de error explícitos y estructurados (por ejemplo con `thiserror`).
  - En `backend`, mapear errores de dominio a respuestas HTTP claras mediante `IntoResponse`.
- **Determinismo en `core-engine`:**
  - Toda lógica de generación de stats y futuro combate debe ser **100% determinista**: dada la misma entrada, debe producir exactamente la misma salida en cualquier arquitectura.
  - Usar `BTreeMap` / `BTreeSet` (u ordenación explícita) en lugar de `HashMap` / `HashSet` cuando el orden de iteración pueda afectar cálculos o serialización.
  - Prohibido leer la hora del sistema (`SystemTime::now()`) o generar aleatoriedad sin semilla dentro del motor puro; los timestamps o semillas deben recibirse explícitamente como parámetros de entrada.
- **Documentación y Limpieza:**
  - Todos los `struct`, `enum`, `trait` y funciones públicas deben tener docstrings (`///`).
  - Evitar lógica duplicada; extraer helpers pequeños y usar early returns.
- **Pruebas Unitarias (Obligatorias):**
  - Todo módulo implementado en Rust debe incluir un módulo `#[cfg(test)] mod tests` ejecutable con `cargo test`.
  - Cobertura prioritaria en Fase 1: fórmulas procedurales de `taxonomy/`, manejo de errores y prioridad de capas en `mods/`, y validación de firmas/integridad en `crypto/`.
  - En Fase 2, el motor `combat/` exigirá tests de determinismo (misma secuencia de acciones + misma semilla = idéntico estado final).

### B. SQL y PostGIS (`backend/migrations/`)

- Sistema de coordenadas estándar: **`SRID 4326` (WGS 84)** para puntos GPS y polígonos de hábitat/santuario.
- Indexar todas las columnas espaciales con índices **`GIST`**.
- Las migraciones son inmutables una vez aplicadas; cualquier cambio requiere una nueva migración secuencial o con timestamp en `backend/migrations/`.
- Toda tabla de polígonos geográficos debe conservar metadatos de origen/licencia (ej. GBIF, iNaturalist, OSM, registro oficial de UMA/zoológico).

### C. Android (`mobile-android/`)

- Lenguaje: **Kotlin** siguiendo las guías oficiales de estilo (`ktlint` / Kotlin Coding Conventions).
- UI: **Jetpack Compose** con estado unidireccional (ViewModel + `StateFlow`).
- **Regla Anti-Trampas (Inviolable):**
  - El flujo de captura competitiva solo puede usar **CameraX** en vivo.
  - Prohibido aceptar imágenes de galería/almacenamiento externo para generar cartas competitivas o intercambiables.
  - El payload firmado debe incluir imagen capturada, timestamp, coordenadas GPS y lecturas de sensores (acelerómetro/giroscopio) junto con la metadata de dispositivo necesaria para verificación.

### D. Mods Declarativos (`mods/`)

- Formato base: `mod.toml` (manifiesto) y archivos por especie/regla en `TOML` (y `GeoJSON`/`JSON` para polígonos de expedición).
- Todo pack en `mods/` debe ser validado automáticamente por `core-engine::mods` en los tests del repositorio.
- Prioridad de resolución de reglas:
  `[Reglas Procedurales Base] -> [Pack Oficial base-pack] -> [Mods Comunitarios]`.

---

## 4. Comandos de Compilación, Lint y Test

> **Estado actual de inicialización:** Los manifiestos `Cargo.toml`, `docker-compose.yml` y el proyecto Gradle en `mobile-android/` se crearán una vez que el Arquitecto apruebe el primer contrato de datos. En el entorno actual están disponibles `rustc`, `cargo`, `git` y `docker` (PostgreSQL/PostGIS y herramientas CLI adicionales se ejecutarán vía Docker o contenedores/wrappers cuando se configuren).

### Workspace Rust (a partir de la creación de `Cargo.toml`)

```bash
# Verificar formato
cargo fmt --all -- --check

# Linting estricto
cargo clippy --workspace --all-targets -- -D warnings

# Compilar todo el workspace
cargo build --workspace

# Ejecutar todos los tests unitarios y de integración
cargo test --workspace

# Ejecutar solo tests del motor core (Fase 1)
cargo test -p biodex-core

# Ejecutar solo tests del backend
cargo test -p biodex-backend
```

*(Nota: los nombres finales de los paquetes `-p biodex-core` / `-p biodex-backend` se confirmarán en el `Cargo.toml` del workspace).*

### Base de Datos y Migraciones (`backend/`)

```bash
# Levantar PostgreSQL + PostGIS local (cuando se agregue docker-compose.yml)
docker compose up -d postgres

# Ejecutar migraciones con sqlx-cli (requiere DATABASE_URL configurado)
sqlx migrate run --source backend/migrations

# Verificar esquema offline SQLx antes de commit (cuando aplique)
cargo sqlx prepare --workspace -- --all-targets
```

### Cliente Android (`mobile-android/`)

```bash
cd mobile-android

# Tests unitarios
./gradlew testDebugUnitTest

# Compilar APK debug
./gradlew assembleDebug

# Análisis estático / Lint
./gradlew lint
```

---

## 5. Checklist de Entrega para Agentes IA

Antes de dar por finalizada cualquier tarea de implementación:

1. ¿El diseño de interfaces, structs y esquema SQL fue aprobado previamente por el Arquitecto?
2. ¿El cambio se mantiene estrictamente dentro del alcance de la fase actual (Fase 1 MVP)?
3. ¿Se agregaron o actualizaron tests unitarios que cubran casos límite y errores?
4. ¿Pasaron `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` y `cargo test` sin tests ignorados ni aserciones relajadas?
5. ¿La documentación (`ARCHITECTURE.md`, `AGENTS.md`, `docs/`) refleja exactamente lo implementado?
