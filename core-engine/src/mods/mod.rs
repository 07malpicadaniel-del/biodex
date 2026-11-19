//! Declarative TOML mod parser, validator, and layered override registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::taxonomy::{
    BiologicalTraits, CaptureVariant, CardMove, SpeciesCard, TaxonId, generate_card,
};

/// Current supported manifest schema version for Phase 1 declarative packs.
pub const SUPPORTED_MOD_SCHEMA_VERSION: u32 = 1;

fn default_species_paths() -> Vec<String> {
    vec!["species/*.toml".to_string()]
}

/// Precedence layer for mod loading (`OfficialBase` applies before `Community`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModLayer {
    /// Official baseline pack (`mods/base-pack/`).
    OfficialBase,
    /// Third-party community packs and regional expeditions.
    Community,
}

/// Declarative metadata loaded from a pack's `mod.toml` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModManifest {
    /// Schema version (must equal [`SUPPORTED_MOD_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// Unique lowercase kebab-case pack identifier (e.g., `"base-pack"`).
    pub id: String,
    /// Human-readable pack title.
    pub name: String,
    /// Semantic version (`major.minor.patch`).
    pub version: String,
    /// Pack authors or maintainers.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Resolution layer (`official_base` or `community`).
    pub layer: ModLayer,
    /// Ordering priority within the same [`ModLayer`] (higher overrides lower).
    #[serde(default)]
    pub priority: u32,
    /// Optional short description of the pack.
    pub description: Option<String>,
    /// Relative glob patterns for species override TOML files.
    #[serde(default = "default_species_paths")]
    pub species_paths: Vec<String>,
    /// Optional relative glob patterns for declarative Almanac species dictionary TOML files.
    #[serde(default)]
    pub almanac_paths: Vec<String>,
}

/// High-level grouping used in the Almanac dictionary UI and API filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlmanacCategory {
    /// Birds (`Aves`).
    Birds,
    /// Mammals (`Mammalia`).
    Mammals,
    /// Reptiles and amphibians (`Reptilia`, `Amphibia`).
    ReptilesAmphibians,
    /// Insects and arachnids (`Insecta`, `Arachnida`).
    InsectsArachnids,
    /// Marine and freshwater aquatic fauna.
    Aquatic,
}

impl AlmanacCategory {
    /// Returns the canonical snake_case identifier stored in PostgreSQL and HTTP queries.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Birds => "birds",
            Self::Mammals => "mammals",
            Self::ReptilesAmphibians => "reptiles_amphibians",
            Self::InsectsArachnids => "insects_arachnids",
            Self::Aquatic => "aquatic",
        }
    }
}

/// Declarative encyclopedia entry loaded from `almanac/*.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlmanacEntry {
    /// Core biological traits used by the deterministic procedural card generator.
    #[serde(flatten)]
    pub traits: BiologicalTraits,
    /// High-level Almanac category for browsing and filtering.
    pub category: AlmanacCategory,
    /// Concise encyclopedic description shown in the Almanac dictionary.
    pub summary: String,
    /// Optional human-readable region or environment hint.
    pub region_hint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AlmanacFile {
    #[serde(default)]
    entries: Vec<AlmanacEntry>,
}

/// Per-species declarative override loaded from `species/*.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesOverride {
    /// Canonical binomial scientific name to match against generated cards.
    pub scientific_name: String,
    /// Optional replacement for the card's display name.
    pub display_name_override: Option<String>,
    /// Optional relative asset/sprite override path.
    pub art_override: Option<String>,
    /// Optional custom moves replacing procedural moves when non-empty.
    #[serde(default)]
    pub moves_override: Vec<CardMove>,
}

/// Validated mod pack containing its manifest, species overrides, and Almanac entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModPack {
    /// Validated pack manifest.
    pub manifest: ModManifest,
    /// Species overrides keyed deterministically by trimmed `scientific_name`.
    pub overrides: BTreeMap<String, SpeciesOverride>,
    /// Validated Almanac catalog entries keyed deterministically by [`TaxonId`].
    pub almanac_entries: BTreeMap<TaxonId, AlmanacEntry>,
}

/// Errors produced when parsing, validating, or registering mod packs.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModError {
    /// Unsupported manifest schema version.
    #[error("unsupported mod schema_version {0} (expected {SUPPORTED_MOD_SCHEMA_VERSION})")]
    UnsupportedSchemaVersion(u32),
    /// Invalid or malformed manifest field.
    #[error("invalid manifest field '{field}': {reason}")]
    InvalidManifestField {
        /// Name of the offending field.
        field: &'static str,
        /// Validation failure explanation.
        reason: String,
    },
    /// Invalid species override entry.
    #[error("invalid species override for '{species}': {reason}")]
    InvalidSpeciesOverride {
        /// Scientific name (or placeholder if missing).
        species: String,
        /// Validation failure explanation.
        reason: String,
    },
    /// Invalid Almanac dictionary entry.
    #[error("invalid almanac entry for '{species}': {reason}")]
    InvalidAlmanacEntry {
        /// Scientific name (or placeholder if missing).
        species: String,
        /// Validation failure explanation.
        reason: String,
    },
    /// Duplicate species override inside a single pack.
    #[error("duplicate species override for '{0}' inside the same mod pack")]
    DuplicateSpeciesOverride(String),
    /// Duplicate Almanac taxon inside a single pack.
    #[error("duplicate almanac entry for taxon '{0:?}' inside the same mod pack")]
    DuplicateAlmanacTaxon(TaxonId),
    /// Duplicate mod pack ID registered in [`ModRegistry`].
    #[error("duplicate mod pack id '{0}'")]
    DuplicateModId(String),
    /// Unsafe path traversal or invalid glob pattern.
    #[error("unsafe or invalid mod relative path: '{0}'")]
    UnsafePath(String),
    /// TOML deserialization failure.
    #[error("TOML parse error: {0}")]
    TomlParse(String),
    /// Filesystem or glob resolution failure.
    #[error("filesystem error: {0}")]
    Io(String),
}

impl ModPack {
    /// Parses and validates a [`ModPack`] purely from TOML strings in memory (without Almanac files).
    pub fn from_toml_strs<'a>(
        manifest_toml: &str,
        species_tomls: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ModError> {
        Self::from_toml_strs_with_almanac(manifest_toml, species_tomls, std::iter::empty())
    }

    /// Parses and validates a [`ModPack`] including both species overrides and Almanac dictionary TOMLs.
    pub fn from_toml_strs_with_almanac<'a>(
        manifest_toml: &str,
        species_tomls: impl IntoIterator<Item = &'a str>,
        almanac_tomls: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, ModError> {
        let manifest: ModManifest =
            toml::from_str(manifest_toml).map_err(|err| ModError::TomlParse(err.to_string()))?;
        validate_manifest(&manifest)?;

        let mut overrides = BTreeMap::new();
        for species_toml in species_tomls {
            let entry: SpeciesOverride =
                toml::from_str(species_toml).map_err(|err| ModError::TomlParse(err.to_string()))?;
            let normalized = validate_and_normalize_override(entry)?;
            let key = normalized.scientific_name.clone();
            if overrides.insert(key.clone(), normalized).is_some() {
                return Err(ModError::DuplicateSpeciesOverride(key));
            }
        }

        let mut almanac_entries = BTreeMap::new();
        for almanac_toml in almanac_tomls {
            let file: AlmanacFile =
                toml::from_str(almanac_toml).map_err(|err| ModError::TomlParse(err.to_string()))?;
            for entry in file.entries {
                let normalized = validate_and_normalize_almanac_entry(entry)?;
                let key = normalized.traits.taxon_id;
                if almanac_entries.insert(key, normalized).is_some() {
                    return Err(ModError::DuplicateAlmanacTaxon(key));
                }
            }
        }

        Ok(Self {
            manifest,
            overrides,
            almanac_entries,
        })
    }

    /// Loads, parses, and validates a [`ModPack`] from a directory containing `mod.toml`.
    pub fn load_from_dir(pack_dir: &Path) -> Result<Self, ModError> {
        let manifest_path = pack_dir.join("mod.toml");
        let manifest_content = fs::read_to_string(&manifest_path).map_err(|err| {
            ModError::Io(format!(
                "failed to read '{}': {err}",
                manifest_path.display()
            ))
        })?;

        let manifest: ModManifest = toml::from_str(&manifest_content)
            .map_err(|err| ModError::TomlParse(err.to_string()))?;
        validate_manifest(&manifest)?;

        let species_files = resolve_pack_files(pack_dir, &manifest.species_paths)?;
        let mut species_contents = Vec::with_capacity(species_files.len());
        for path in species_files {
            let content = fs::read_to_string(&path).map_err(|err| {
                ModError::Io(format!("failed to read '{}': {err}", path.display()))
            })?;
            species_contents.push(content);
        }

        let almanac_files = resolve_pack_files(pack_dir, &manifest.almanac_paths)?;
        let mut almanac_contents = Vec::with_capacity(almanac_files.len());
        for path in almanac_files {
            let content = fs::read_to_string(&path).map_err(|err| {
                ModError::Io(format!("failed to read '{}': {err}", path.display()))
            })?;
            almanac_contents.push(content);
        }

        Self::from_toml_strs_with_almanac(
            &manifest_content,
            species_contents.iter().map(String::as_str),
            almanac_contents.iter().map(String::as_str),
        )
    }
}

/// Ordered registry of loaded mod packs supporting layered override resolution.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ModRegistry {
    packs: Vec<ModPack>,
}

impl ModRegistry {
    /// Creates an empty [`ModRegistry`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a validated [`ModPack`], rejecting duplicate pack IDs and preserving
    /// deterministic layer/priority order.
    pub fn register(&mut self, pack: ModPack) -> Result<(), ModError> {
        if self
            .packs
            .iter()
            .any(|existing| existing.manifest.id == pack.manifest.id)
        {
            return Err(ModError::DuplicateModId(pack.manifest.id));
        }

        self.packs.push(pack);
        self.packs.sort_by(|a, b| {
            (a.manifest.layer, a.manifest.priority, &a.manifest.id).cmp(&(
                b.manifest.layer,
                b.manifest.priority,
                &b.manifest.id,
            ))
        });
        Ok(())
    }

    /// Loads and registers a pack directory containing `mod.toml`.
    pub fn load_dir(&mut self, pack_dir: &Path) -> Result<(), ModError> {
        let pack = ModPack::load_from_dir(pack_dir)?;
        self.register(pack)
    }

    /// Returns registered packs in ascending application order (last applied wins).
    #[must_use]
    pub fn packs(&self) -> &[ModPack] {
        &self.packs
    }

    /// Applies matching overrides across all registered layers onto a generated [`SpeciesCard`].
    #[must_use]
    pub fn apply_to_card(&self, mut card: SpeciesCard) -> SpeciesCard {
        for pack in &self.packs {
            if let Some(species_override) = pack.overrides.get(&card.scientific_name) {
                if let Some(display_name) = &species_override.display_name_override {
                    card.display_name = display_name.clone();
                }
                if let Some(art_path) = &species_override.art_override {
                    card.art_override = Some(art_path.clone());
                }
                if !species_override.moves_override.is_empty() {
                    card.moves = species_override.moves_override.clone();
                }
                card.applied_pack = pack.manifest.id.clone();
            }
        }
        card
    }

    /// Returns merged Almanac entries across all registered packs in deterministic
    /// `(category, common_name_or_scientific, scientific_name)` order.
    #[must_use]
    pub fn almanac_entries(&self) -> Vec<AlmanacEntry> {
        let mut merged = BTreeMap::new();
        for pack in &self.packs {
            for (taxon_id, entry) in &pack.almanac_entries {
                merged.insert(*taxon_id, entry.clone());
            }
        }
        let mut list: Vec<AlmanacEntry> = merged.into_values().collect();
        list.sort_by(|a, b| {
            let a_label = a
                .traits
                .common_name
                .as_deref()
                .unwrap_or(&a.traits.scientific_name);
            let b_label = b
                .traits
                .common_name
                .as_deref()
                .unwrap_or(&b.traits.scientific_name);
            (a.category, a_label, &a.traits.scientific_name).cmp(&(
                b.category,
                b_label,
                &b.traits.scientific_name,
            ))
        });
        list
    }
}

fn validate_manifest(manifest: &ModManifest) -> Result<(), ModError> {
    if manifest.schema_version != SUPPORTED_MOD_SCHEMA_VERSION {
        return Err(ModError::UnsupportedSchemaVersion(manifest.schema_version));
    }
    if !is_valid_slug(&manifest.id) {
        return Err(ModError::InvalidManifestField {
            field: "id",
            reason:
                "must be non-empty lowercase ASCII alphanumeric or '-' without leading/trailing '-'"
                    .to_string(),
        });
    }
    if manifest.name.trim().is_empty() {
        return Err(ModError::InvalidManifestField {
            field: "name",
            reason: "must not be empty".to_string(),
        });
    }
    if !is_valid_semver(&manifest.version) {
        return Err(ModError::InvalidManifestField {
            field: "version",
            reason: "must follow major.minor.patch numeric format".to_string(),
        });
    }
    for pattern in &manifest.species_paths {
        validate_relative_path(pattern)?;
    }
    for pattern in &manifest.almanac_paths {
        validate_relative_path(pattern)?;
    }
    Ok(())
}

fn validate_and_normalize_almanac_entry(mut entry: AlmanacEntry) -> Result<AlmanacEntry, ModError> {
    entry.traits.scientific_name = entry.traits.scientific_name.trim().to_string();
    entry.traits.phylum = entry.traits.phylum.trim().to_string();
    entry.traits.taxonomic_class = entry.traits.taxonomic_class.trim().to_string();
    entry.summary = entry.summary.trim().to_string();

    if let Some(common_name) = &mut entry.traits.common_name {
        let trimmed = common_name.trim().to_string();
        if trimmed.is_empty() {
            entry.traits.common_name = None;
        } else {
            *common_name = trimmed;
        }
    }
    if let Some(region) = &mut entry.region_hint {
        let trimmed = region.trim().to_string();
        if trimmed.is_empty() {
            entry.region_hint = None;
        } else {
            *region = trimmed;
        }
    }

    let species_label = if entry.traits.scientific_name.is_empty() {
        "<empty>".to_string()
    } else {
        entry.traits.scientific_name.clone()
    };

    if entry.traits.phylum.is_empty() || entry.traits.taxonomic_class.is_empty() {
        return Err(ModError::InvalidAlmanacEntry {
            species: species_label,
            reason: "phylum and taxonomic_class must not be empty".to_string(),
        });
    }
    if entry.summary.is_empty() {
        return Err(ModError::InvalidAlmanacEntry {
            species: species_label,
            reason: "summary must not be empty".to_string(),
        });
    }

    generate_card(&entry.traits, CaptureVariant::Wild).map_err(|err| {
        ModError::InvalidAlmanacEntry {
            species: species_label,
            reason: err.to_string(),
        }
    })?;

    Ok(entry)
}

fn validate_and_normalize_override(
    mut entry: SpeciesOverride,
) -> Result<SpeciesOverride, ModError> {
    entry.scientific_name = entry.scientific_name.trim().to_string();
    if entry.scientific_name.is_empty() {
        return Err(ModError::InvalidSpeciesOverride {
            species: "<empty>".to_string(),
            reason: "scientific_name must not be empty".to_string(),
        });
    }

    if let Some(display_name) = &mut entry.display_name_override {
        let trimmed = display_name.trim().to_string();
        if trimmed.is_empty() {
            return Err(ModError::InvalidSpeciesOverride {
                species: entry.scientific_name,
                reason: "display_name_override must not be blank when present".to_string(),
            });
        }
        *display_name = trimmed;
    }

    if let Some(art_path) = &entry.art_override {
        validate_relative_path(art_path)?;
    }

    for mv in &entry.moves_override {
        if mv.id.trim().is_empty() || mv.name.trim().is_empty() {
            return Err(ModError::InvalidSpeciesOverride {
                species: entry.scientific_name,
                reason: "move id and name must not be empty".to_string(),
            });
        }
        if !(1..=10).contains(&mv.energy_cost) {
            return Err(ModError::InvalidSpeciesOverride {
                species: entry.scientific_name,
                reason: format!("move '{}' energy_cost must be within 1..=10", mv.id),
            });
        }
    }

    Ok(entry)
}

fn validate_relative_path(raw: &str) -> Result<(), ModError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ModError::UnsafePath(raw.to_string()));
    }
    let path = Path::new(trimmed);
    if path.is_absolute() {
        return Err(ModError::UnsafePath(raw.to_string()));
    }
    for component in path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ModError::UnsafePath(raw.to_string()));
            }
        }
    }
    Ok(())
}

fn resolve_pack_files(pack_dir: &Path, patterns: &[String]) -> Result<Vec<PathBuf>, ModError> {
    let canonical_root = fs::canonicalize(pack_dir).map_err(|err| {
        ModError::Io(format!(
            "failed to canonicalize '{}': {err}",
            pack_dir.display()
        ))
    })?;

    let mut unique_paths = BTreeSet::new();
    for pattern in patterns {
        validate_relative_path(pattern)?;
        let full_pattern = pack_dir.join(pattern);
        let pattern_str = full_pattern
            .to_str()
            .ok_or_else(|| ModError::UnsafePath(pattern.clone()))?;

        let entries = glob::glob(pattern_str)
            .map_err(|err| ModError::UnsafePath(format!("{pattern}: {err}")))?;

        for item in entries {
            let path = item.map_err(|err| ModError::Io(err.to_string()))?;
            let canonical_file = fs::canonicalize(&path).map_err(|err| {
                ModError::Io(format!(
                    "failed to canonicalize '{}': {err}",
                    path.display()
                ))
            })?;
            if !canonical_file.starts_with(&canonical_root) {
                return Err(ModError::UnsafePath(pattern.clone()));
            }
            if canonical_file.is_file() {
                unique_paths.insert(canonical_file);
            }
        }
    }

    Ok(unique_paths.into_iter().collect())
}

fn is_valid_slug(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_valid_semver(value: &str) -> bool {
    let mut parts = value.split('.');
    let Some(major) = parts.next() else {
        return false;
    };
    let Some(minor) = parts.next() else {
        return false;
    };
    let Some(patch) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && [major, minor, patch]
            .into_iter()
            .all(|segment| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::taxonomy::{
        BiologicalTraits, CaptureVariant, DefenseTrait, Diet, Habitat, IucnStatus, TaxonId,
        generate_card,
    };

    const BASE_MANIFEST: &str = r#"
schema_version = 1
id = "base-pack"
name = "BioDex Official Base Pack"
version = "0.1.0"
authors = ["BioDex Core"]
layer = "official_base"
priority = 0
species_paths = ["species/*.toml"]
"#;

    const SQUID_OVERRIDE: &str = r#"
scientific_name = "Architeuthis dux"
display_name_override = "Calamar Gigante Abisal"
art_override = "sprites/architeuthis_dux.png"

[[moves_override]]
id = "abyssal_embrace"
name = "Abrazo Abisal"
energy_cost = 4
power = 95
tag = "abyssal_grapple"
"#;

    fn sample_squid_card() -> SpeciesCard {
        let traits = BiologicalTraits {
            taxon_id: TaxonId::Gbif(5_189_893),
            scientific_name: "Architeuthis dux".to_string(),
            common_name: Some("Calamar Gigante".to_string()),
            phylum: "Mollusca".to_string(),
            taxonomic_class: "Cephalopoda".to_string(),
            average_mass_grams: 275_000,
            diet: Diet::Carnivore,
            habitat: Habitat::Abyssal,
            defenses: BTreeSet::from([DefenseTrait::Ink]),
            iucn_status: IucnStatus::Lc,
        };
        generate_card(&traits, CaptureVariant::Wild).unwrap()
    }

    #[test]
    fn parses_valid_pack_and_applies_species_override() {
        let pack = ModPack::from_toml_strs(BASE_MANIFEST, [SQUID_OVERRIDE]).unwrap();
        let mut registry = ModRegistry::new();
        registry.register(pack).unwrap();

        let card = registry.apply_to_card(sample_squid_card());
        assert_eq!(card.applied_pack, "base-pack");
        assert_eq!(card.display_name, "Calamar Gigante Abisal");
        assert_eq!(
            card.art_override.as_deref(),
            Some("sprites/architeuthis_dux.png")
        );
        assert_eq!(card.moves.len(), 1);
        assert_eq!(card.moves[0].name, "Abrazo Abisal");
    }

    #[test]
    fn community_layer_overrides_official_base_regardless_of_registration_order() {
        let base_pack = ModPack::from_toml_strs(BASE_MANIFEST, [SQUID_OVERRIDE]).unwrap();
        let community_manifest = r#"
schema_version = 1
id = "veracruz-expedition"
name = "Expedicion Veracruz"
version = "0.2.0"
layer = "community"
priority = 1
"#;
        let community_override = r#"
scientific_name = "Architeuthis dux"
display_name_override = "Kraken del Golfo"
"#;
        let community_pack =
            ModPack::from_toml_strs(community_manifest, [community_override]).unwrap();

        // Register community BEFORE official base to verify deterministic sorting.
        let mut registry = ModRegistry::new();
        registry.register(community_pack).unwrap();
        registry.register(base_pack).unwrap();

        let card = registry.apply_to_card(sample_squid_card());
        assert_eq!(card.applied_pack, "veracruz-expedition");
        assert_eq!(card.display_name, "Kraken del Golfo");
        // Base pack moves/art remain intact when community override only alters display_name.
        assert_eq!(card.moves[0].name, "Abrazo Abisal");
        assert_eq!(
            card.art_override.as_deref(),
            Some("sprites/architeuthis_dux.png")
        );
    }

    #[test]
    fn rejects_unsupported_schema_and_path_traversal() {
        let bad_schema = BASE_MANIFEST.replace("schema_version = 1", "schema_version = 99");
        assert_eq!(
            ModPack::from_toml_strs(&bad_schema, []),
            Err(ModError::UnsupportedSchemaVersion(99))
        );

        let traversal_manifest = BASE_MANIFEST.replace("species/*.toml", "../secrets/*.toml");
        assert_eq!(
            ModPack::from_toml_strs(&traversal_manifest, []),
            Err(ModError::UnsafePath("../secrets/*.toml".to_string()))
        );
    }

    #[test]
    fn rejects_duplicate_mod_ids_and_invalid_move_cost() {
        let pack_a = ModPack::from_toml_strs(BASE_MANIFEST, []).unwrap();
        let pack_b = ModPack::from_toml_strs(BASE_MANIFEST, []).unwrap();

        let mut registry = ModRegistry::new();
        registry.register(pack_a).unwrap();
        assert_eq!(
            registry.register(pack_b),
            Err(ModError::DuplicateModId("base-pack".to_string()))
        );

        let invalid_cost = SQUID_OVERRIDE.replace("energy_cost = 4", "energy_cost = 15");
        assert!(matches!(
            ModPack::from_toml_strs(BASE_MANIFEST, [invalid_cost.as_str()]),
            Err(ModError::InvalidSpeciesOverride { .. })
        ));
    }

    #[test]
    fn loads_official_base_pack_from_repository_directory() {
        let pack_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods/base-pack");
        let mut registry = ModRegistry::new();
        registry.load_dir(&pack_dir).unwrap();

        assert_eq!(registry.packs().len(), 1);
        assert_eq!(registry.packs()[0].manifest.id, "base-pack");

        let card = registry.apply_to_card(sample_squid_card());
        assert_eq!(card.applied_pack, "base-pack");
        assert_eq!(card.moves[0].name, "Abrazo Abisal");
    }

    #[test]
    fn rejects_duplicate_species_in_same_pack_and_invalid_manifest_fields() {
        assert_eq!(
            ModPack::from_toml_strs(BASE_MANIFEST, [SQUID_OVERRIDE, SQUID_OVERRIDE]),
            Err(ModError::DuplicateSpeciesOverride(
                "Architeuthis dux".to_string()
            ))
        );

        let bad_slug = BASE_MANIFEST.replace("id = \"base-pack\"", "id = \"Bad_Slug\"");
        assert!(matches!(
            ModPack::from_toml_strs(&bad_slug, []),
            Err(ModError::InvalidManifestField { field: "id", .. })
        ));

        let bad_version = BASE_MANIFEST.replace("version = \"0.1.0\"", "version = \"0.1\"");
        assert!(matches!(
            ModPack::from_toml_strs(&bad_version, []),
            Err(ModError::InvalidManifestField {
                field: "version",
                ..
            })
        ));
    }

    #[test]
    fn leaves_unmatched_species_as_procedural() {
        let pack = ModPack::from_toml_strs(BASE_MANIFEST, [SQUID_OVERRIDE]).unwrap();
        let mut registry = ModRegistry::new();
        registry.register(pack).unwrap();

        let other = BiologicalTraits {
            taxon_id: TaxonId::INaturalist(42),
            scientific_name: "Panthera onca".to_string(),
            common_name: Some("Jaguar".to_string()),
            phylum: "Chordata".to_string(),
            taxonomic_class: "Mammalia".to_string(),
            average_mass_grams: 90_000,
            diet: Diet::Carnivore,
            habitat: Habitat::Terrestrial,
            defenses: BTreeSet::from([DefenseTrait::Speed]),
            iucn_status: IucnStatus::Nt,
        };
        let card = registry.apply_to_card(generate_card(&other, CaptureVariant::Wild).unwrap());
        assert_eq!(card.applied_pack, "procedural");
        assert_eq!(card.display_name, "Jaguar");
    }

    #[test]
    fn loads_all_24_basic_almanac_species_from_base_pack() {
        let pack_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../mods/base-pack");
        let mut registry = ModRegistry::new();
        registry.load_dir(&pack_dir).unwrap();

        let entries = registry.almanac_entries();
        assert_eq!(entries.len(), 24);

        for entry in &entries {
            let card = generate_card(&entry.traits, CaptureVariant::Wild).unwrap();
            assert!((1..=10).contains(&card.deploy_cost));
            assert!(!entry.summary.is_empty());
        }

        assert!(
            entries
                .iter()
                .any(|e| e.traits.scientific_name == "Ambystoma mexicanum"
                    && e.traits.iucn_status == IucnStatus::Cr)
        );
    }

    #[test]
    fn rejects_duplicate_almanac_taxon_and_blank_summary() {
        let almanac_entry = r#"
[[entries]]
taxon_id = { source = "i_naturalist", id = 13858 }
scientific_name = "Passer domesticus"
common_name = "Gorrión Común"
phylum = "Chordata"
taxonomic_class = "Aves"
average_mass_grams = 28
diet = "omnivore"
habitat = "aerial"
defenses = ["speed"]
iucn_status = "LC"
category = "birds"
summary = "Ave urbana adaptable."
"#;
        assert_eq!(
            ModPack::from_toml_strs_with_almanac(BASE_MANIFEST, [], [almanac_entry, almanac_entry]),
            Err(ModError::DuplicateAlmanacTaxon(TaxonId::INaturalist(
                13_858
            )))
        );

        let blank_summary =
            almanac_entry.replace("summary = \"Ave urbana adaptable.\"", "summary = \"   \"");
        assert!(matches!(
            ModPack::from_toml_strs_with_almanac(BASE_MANIFEST, [], [blank_summary.as_str()]),
            Err(ModError::InvalidAlmanacEntry { .. })
        ));
    }
}
