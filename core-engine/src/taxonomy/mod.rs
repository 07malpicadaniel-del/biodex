//! Deterministic taxonomic traits, card structures, and procedural stat generation.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// External taxonomic identifier returned by vision/taxonomy providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "source", content = "id", rename_all = "snake_case")]
pub enum TaxonId {
    /// Global Biodiversity Information Facility (GBIF) taxon key.
    Gbif(u64),
    /// iNaturalist taxon identifier.
    INaturalist(u64),
}

/// Primary trophic diet of a species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Diet {
    /// Feeds primarily on other animals.
    Carnivore,
    /// Feeds primarily on plant matter or algae.
    Herbivore,
    /// Feeds on both animal and plant matter.
    Omnivore,
    /// Feeds on decaying organic matter.
    Detritivore,
}

/// Primary ecological habitat used for affinity and stat weighting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Habitat {
    /// Deep-ocean benthic or bathypelagic zone.
    Abyssal,
    /// Coastal or open-ocean saltwater environment.
    Marine,
    /// Rivers, lakes, waterfalls, and wetlands.
    Freshwater,
    /// Land biomes (forests, deserts, grasslands, mountains).
    Terrestrial,
    /// Primarily aerial or arboreal flight-capable species.
    Aerial,
}

/// Natural biological defense mechanisms that influence stats and procedural moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefenseTrait {
    /// Biological toxins or venomous apparatus.
    Venom,
    /// Cryptic coloration or mimicry.
    Camouflage,
    /// Exoskeleton, carapace, or bony armor.
    ShellArmor,
    /// Quills, spines, or stinging setae.
    Spines,
    /// Ink cloud or distracting defensive discharge.
    Ink,
    /// High-acceleration evasion reflex.
    Speed,
}

/// IUCN Red List conservation category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum IucnStatus {
    /// Least Concern.
    Lc,
    /// Near Threatened.
    Nt,
    /// Vulnerable.
    Vu,
    /// Endangered.
    En,
    /// Critically Endangered.
    Cr,
    /// Not evaluated or data deficient; maps conservatively to Common.
    Unknown,
}

/// Card rarity derived deterministically from [`IucnStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rarity {
    /// Common (`LC` / `Unknown`).
    Common,
    /// Uncommon (`NT`).
    Uncommon,
    /// Rare (`VU`).
    Rare,
    /// Epic (`EN`).
    Epic,
    /// Mythic (`CR`).
    Mythic,
}

/// Provenance variant assigned by PostGIS geospatial validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureVariant {
    /// Captured in natural species range (100% stat potential).
    Wild,
    /// Captured inside a registered zoo, aquarium, or sanctuary polygon (85% stat multiplier).
    Sanctuary,
}

impl CaptureVariant {
    /// Returns the approved integer percentage stat multiplier for this capture variant.
    #[must_use]
    pub const fn stat_multiplier_pct(self) -> u16 {
        match self {
            Self::Wild => 100,
            Self::Sanctuary => 85,
        }
    }
}

/// Biological input attributes gathered from taxonomic metadata sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiologicalTraits {
    /// External provider taxon identifier.
    pub taxon_id: TaxonId,
    /// Binomial scientific name (e.g., `"Architeuthis dux"`).
    pub scientific_name: String,
    /// Optional localized or vernacular name.
    pub common_name: Option<String>,
    /// Taxonomic phylum (e.g., `"Chordata"`, `"Mollusca"`).
    pub phylum: String,
    /// Taxonomic class (e.g., `"Mammalia"`, `"Cephalopoda"`).
    pub taxonomic_class: String,
    /// Average adult body mass in grams (integer for deterministic calculation).
    pub average_mass_grams: u64,
    /// Trophic classification.
    pub diet: Diet,
    /// Primary habitat classification.
    pub habitat: Habitat,
    /// Deterministic ordered set of natural defensive traits.
    pub defenses: BTreeSet<DefenseTrait>,
    /// IUCN Red List status.
    pub iucn_status: IucnStatus,
}

/// Core integer combat stats of a generated card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardStats {
    /// Health points.
    pub hp: u16,
    /// Offensive strength.
    pub attack: u16,
    /// Damage mitigation.
    pub defense: u16,
    /// Turn-order initiative / agility.
    pub speed: u16,
}

/// Move metadata attached to a [`SpeciesCard`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardMove {
    /// Stable identifier (used by mods and future combat hooks).
    pub id: String,
    /// Human-readable attack name.
    pub name: String,
    /// Energy cost to activate (1..=10).
    pub energy_cost: u8,
    /// Base power value.
    pub power: u16,
    /// Optional semantic tag (e.g., `"venom"`, `"ambush"`).
    pub tag: Option<String>,
}

/// Generated collectible creature card produced by the core engine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesCard {
    /// External provider taxon identifier.
    pub taxon_id: TaxonId,
    /// Canonical scientific name used for mod override matching.
    pub scientific_name: String,
    /// Display name shown on the card.
    pub display_name: String,
    /// Elemental/ecological affinity derived from primary habitat.
    pub affinity: Habitat,
    /// Rarity tier derived from IUCN Red List status.
    pub rarity: Rarity,
    /// Wild vs. Sanctuary provenance variant.
    pub variant: CaptureVariant,
    /// Competitive deployment energy cost (`1..=10`).
    pub deploy_cost: u8,
    /// Scaled biological stats.
    pub stats: CardStats,
    /// Procedural or mod-overridden moves.
    pub moves: Vec<CardMove>,
    /// Optional asset/sprite override path provided by a mod.
    pub art_override: Option<String>,
    /// Identifier of the highest-priority layer applied (`"procedural"` or pack ID).
    pub applied_pack: String,
}

/// Validation errors returned during procedural card generation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaxonomyError {
    /// Scientific name was empty or whitespace-only.
    #[error("scientific_name must not be empty")]
    EmptyScientificName,
    /// Average mass must be at least 1 gram.
    #[error("average_mass_grams must be at least 1 gram")]
    InvalidMass,
}

/// Maps an [`IucnStatus`] to its corresponding [`Rarity`].
#[must_use]
pub const fn rarity_from_iucn(status: IucnStatus) -> Rarity {
    match status {
        IucnStatus::Lc | IucnStatus::Unknown => Rarity::Common,
        IucnStatus::Nt => Rarity::Uncommon,
        IucnStatus::Vu => Rarity::Rare,
        IucnStatus::En => Rarity::Epic,
        IucnStatus::Cr => Rarity::Mythic,
    }
}

/// Computes the competitive deployment energy cost (`1..=10`) from average mass in grams.
#[must_use]
pub const fn compute_deploy_cost(average_mass_grams: u64) -> u8 {
    if average_mass_grams == 0 {
        return 1;
    }
    let mass_log2 = average_mass_grams.ilog2();
    let capped_log2 = if mass_log2 > 28 { 28 } else { mass_log2 };
    let raw = 1 + (capped_log2 * 3 + 4) / 8;
    if raw > 10 { 10 } else { raw as u8 }
}

/// Deterministically generates a [`SpeciesCard`] from [`BiologicalTraits`] and [`CaptureVariant`].
pub fn generate_card(
    traits: &BiologicalTraits,
    variant: CaptureVariant,
) -> Result<SpeciesCard, TaxonomyError> {
    let scientific_name = traits.scientific_name.trim();
    if scientific_name.is_empty() {
        return Err(TaxonomyError::EmptyScientificName);
    }
    if traits.average_mass_grams == 0 {
        return Err(TaxonomyError::InvalidMass);
    }

    let rarity = rarity_from_iucn(traits.iucn_status);
    let deploy_cost = compute_deploy_cost(traits.average_mass_grams);
    let stats = compute_stats(traits, rarity, variant);
    let moves = build_procedural_moves(traits, stats.attack, deploy_cost);

    let display_name = traits
        .common_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(scientific_name)
        .to_string();

    Ok(SpeciesCard {
        taxon_id: traits.taxon_id,
        scientific_name: scientific_name.to_string(),
        display_name,
        affinity: traits.habitat,
        rarity,
        variant,
        deploy_cost,
        stats,
        moves,
        art_override: None,
        applied_pack: "procedural".to_string(),
    })
}

fn compute_stats(traits: &BiologicalTraits, rarity: Rarity, variant: CaptureVariant) -> CardStats {
    let mass_log2 = traits.average_mass_grams.ilog2().min(28);

    let mut hp = 40 + mass_log2 * 10;
    let mut attack = 18 + mass_log2 * 5;
    let mut defense = 16 + mass_log2 * 5;
    let mut speed = 42 + 24_u32.saturating_sub(mass_log2);

    apply_diet_modifiers(traits.diet, &mut hp, &mut attack, &mut defense, &mut speed);
    apply_habitat_modifiers(
        traits.habitat,
        &mut hp,
        &mut attack,
        &mut defense,
        &mut speed,
    );
    for defense_trait in &traits.defenses {
        apply_defense_modifiers(*defense_trait, &mut attack, &mut defense, &mut speed);
    }

    let rarity_pct = rarity_bonus_pct(rarity);
    let variant_pct = u32::from(variant.stat_multiplier_pct());

    CardStats {
        hp: scale_stat(hp, rarity_pct, variant_pct),
        attack: scale_stat(attack, rarity_pct, variant_pct),
        defense: scale_stat(defense, rarity_pct, variant_pct),
        speed: scale_stat(speed, rarity_pct, variant_pct),
    }
}

const fn apply_diet_modifiers(
    diet: Diet,
    hp: &mut u32,
    attack: &mut u32,
    defense: &mut u32,
    speed: &mut u32,
) {
    match diet {
        Diet::Carnivore => {
            *attack += 12;
            *speed += 8;
        }
        Diet::Herbivore => {
            *hp += 20;
            *defense += 10;
        }
        Diet::Omnivore => {
            *hp += 10;
            *attack += 6;
            *defense += 6;
            *speed += 4;
        }
        Diet::Detritivore => {
            *hp += 15;
            *defense += 14;
            *attack = attack.saturating_sub(4);
        }
    }
}

const fn apply_habitat_modifiers(
    habitat: Habitat,
    hp: &mut u32,
    attack: &mut u32,
    defense: &mut u32,
    speed: &mut u32,
) {
    match habitat {
        Habitat::Abyssal => {
            *hp += 15;
            *defense += 12;
        }
        Habitat::Marine => {
            *hp += 10;
            *speed += 6;
        }
        Habitat::Freshwater => {
            *hp += 8;
            *speed += 8;
        }
        Habitat::Terrestrial => {
            *attack += 6;
            *defense += 6;
        }
        Habitat::Aerial => {
            *speed += 18;
            *hp = hp.saturating_sub(8);
        }
    }
}

const fn apply_defense_modifiers(
    trait_kind: DefenseTrait,
    attack: &mut u32,
    defense: &mut u32,
    speed: &mut u32,
) {
    match trait_kind {
        DefenseTrait::Venom => *attack += 14,
        DefenseTrait::Camouflage => {
            *speed += 10;
            *defense += 4;
        }
        DefenseTrait::ShellArmor => {
            *defense += 20;
            *speed = speed.saturating_sub(8);
        }
        DefenseTrait::Spines => {
            *defense += 10;
            *attack += 6;
        }
        DefenseTrait::Ink => {
            *speed += 8;
            *defense += 6;
        }
        DefenseTrait::Speed => *speed += 16,
    }
}

const fn rarity_bonus_pct(rarity: Rarity) -> u32 {
    match rarity {
        Rarity::Common => 100,
        Rarity::Uncommon => 105,
        Rarity::Rare => 110,
        Rarity::Epic => 115,
        Rarity::Mythic => 122,
    }
}

fn scale_stat(base: u32, rarity_pct: u32, variant_pct: u32) -> u16 {
    let scaled = (base * rarity_pct * variant_pct) / 10_000;
    scaled.clamp(1, u32::from(u16::MAX)) as u16
}

fn build_procedural_moves(
    traits: &BiologicalTraits,
    attack_stat: u16,
    deploy_cost: u8,
) -> Vec<CardMove> {
    let primary_energy = deploy_cost.div_ceil(2).clamp(1, 5);
    let (id, name, tag) = match (traits.diet, traits.habitat) {
        (Diet::Carnivore, Habitat::Abyssal) => (
            "abyssal_strike",
            "Embestida Abisal",
            Some("abyssal".to_string()),
        ),
        (Diet::Carnivore, _) => (
            "predator_bite",
            "Mordida Depredadora",
            Some("predator".to_string()),
        ),
        (Diet::Herbivore, _) => (
            "territorial_charge",
            "Carga Territorial",
            Some("sturdy".to_string()),
        ),
        (Diet::Omnivore, _) => (
            "adaptive_strike",
            "Golpe Adaptativo",
            Some("versatile".to_string()),
        ),
        (Diet::Detritivore, _) => ("scavenger_clamp", "Tenaza Carroñera", None),
    };

    let mut moves = vec![CardMove {
        id: id.to_string(),
        name: name.to_string(),
        energy_cost: primary_energy,
        power: ((u32::from(attack_stat) * 8) / 10).max(10) as u16,
        tag,
    }];

    if let Some(first_defense) = traits.defenses.iter().next() {
        let (def_id, def_name, def_energy, def_power, def_tag) = match first_defense {
            DefenseTrait::Venom => (
                "venom_fang",
                "Picadura Venenosa",
                2,
                (attack_stat / 2).max(12),
                "venom",
            ),
            DefenseTrait::Camouflage => (
                "ambush_pounce",
                "Emboscada Críptica",
                2,
                ((u32::from(attack_stat) * 65) / 100).max(12) as u16,
                "ambush",
            ),
            DefenseTrait::ShellArmor => ("iron_carapace", "Caparazón Férreo", 2, 15, "shield"),
            DefenseTrait::Spines => ("spine_barrage", "Ráfaga de Espinas", 2, 24, "thorns"),
            DefenseTrait::Ink => ("ink_veil", "Nube de Tinta", 1, 12, "evasion"),
            DefenseTrait::Speed => (
                "blitz_dash",
                "Asalto Relámpago",
                1,
                ((u32::from(attack_stat) * 6) / 10).max(10) as u16,
                "priority",
            ),
        };

        moves.push(CardMove {
            id: def_id.to_string(),
            name: def_name.to_string(),
            energy_cost: def_energy,
            power: def_power,
            tag: Some(def_tag.to_string()),
        });
    }

    moves
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_squid_traits() -> BiologicalTraits {
        BiologicalTraits {
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
        }
    }

    fn sample_poison_frog_traits() -> BiologicalTraits {
        BiologicalTraits {
            taxon_id: TaxonId::INaturalist(21_129),
            scientific_name: "Dendrobates tinctorius".to_string(),
            common_name: Some("Rana Dardo".to_string()),
            phylum: "Chordata".to_string(),
            taxonomic_class: "Amphibia".to_string(),
            average_mass_grams: 4,
            diet: Diet::Carnivore,
            habitat: Habitat::Freshwater,
            defenses: BTreeSet::from([DefenseTrait::Venom, DefenseTrait::Camouflage]),
            iucn_status: IucnStatus::Cr,
        }
    }

    #[test]
    fn maps_all_iucn_statuses_to_expected_rarities() {
        assert_eq!(rarity_from_iucn(IucnStatus::Lc), Rarity::Common);
        assert_eq!(rarity_from_iucn(IucnStatus::Unknown), Rarity::Common);
        assert_eq!(rarity_from_iucn(IucnStatus::Nt), Rarity::Uncommon);
        assert_eq!(rarity_from_iucn(IucnStatus::Vu), Rarity::Rare);
        assert_eq!(rarity_from_iucn(IucnStatus::En), Rarity::Epic);
        assert_eq!(rarity_from_iucn(IucnStatus::Cr), Rarity::Mythic);
    }

    #[test]
    fn deploy_cost_scales_across_biological_masses() {
        let frog_cost = compute_deploy_cost(4);
        let squid_cost = compute_deploy_cost(275_000);
        let whale_cost = compute_deploy_cost(100_000_000);

        assert!((2..=3).contains(&frog_cost));
        assert!((8..=9).contains(&squid_cost));
        assert_eq!(whale_cost, 10);
    }

    #[test]
    fn sanctuary_applies_85_percent_multiplier_deterministically() {
        let squid = sample_squid_traits();
        let wild = generate_card(&squid, CaptureVariant::Wild).unwrap();
        let sanctuary = generate_card(&squid, CaptureVariant::Sanctuary).unwrap();

        assert_eq!(wild.deploy_cost, sanctuary.deploy_cost);
        assert_eq!(wild.rarity, Rarity::Common);
        assert!(sanctuary.stats.hp < wild.stats.hp);
        assert_eq!(
            sanctuary.stats.hp,
            ((u32::from(wild.stats.hp) * 85) / 100) as u16
        );
        assert_eq!(
            sanctuary.stats.attack,
            ((u32::from(wild.stats.attack) * 85) / 100) as u16
        );
    }

    #[test]
    fn card_generation_is_deterministic_and_assigns_defense_move() {
        let frog = sample_poison_frog_traits();
        let first = generate_card(&frog, CaptureVariant::Wild).unwrap();
        let second = generate_card(&frog, CaptureVariant::Wild).unwrap();

        assert_eq!(first, second);
        assert_eq!(first.rarity, Rarity::Mythic);
        assert_eq!(first.moves.len(), 2);
        assert_eq!(first.moves[1].id, "venom_fang");
    }

    #[test]
    fn rejects_invalid_mass_or_empty_scientific_name() {
        let mut invalid_name = sample_squid_traits();
        invalid_name.scientific_name = "   ".to_string();
        assert_eq!(
            generate_card(&invalid_name, CaptureVariant::Wild),
            Err(TaxonomyError::EmptyScientificName)
        );

        let mut zero_mass = sample_squid_traits();
        zero_mass.average_mass_grams = 0;
        assert_eq!(
            generate_card(&zero_mass, CaptureVariant::Wild),
            Err(TaxonomyError::InvalidMass)
        );
    }

    #[test]
    fn falls_back_to_scientific_name_and_handles_extreme_masses() {
        let mut minimal = sample_squid_traits();
        minimal.common_name = Some("   ".to_string());
        minimal.average_mass_grams = 1;
        minimal.defenses.clear();

        let tiny_card = generate_card(&minimal, CaptureVariant::Wild).unwrap();
        assert_eq!(tiny_card.display_name, "Architeuthis dux");
        assert_eq!(tiny_card.deploy_cost, 1);
        assert_eq!(tiny_card.moves.len(), 1);

        let mut huge = minimal;
        huge.common_name = None;
        huge.average_mass_grams = u64::MAX;
        huge.diet = Diet::Herbivore;
        huge.habitat = Habitat::Marine;
        huge.defenses = BTreeSet::from([DefenseTrait::ShellArmor, DefenseTrait::Spines]);
        let giant_card = generate_card(&huge, CaptureVariant::Wild).unwrap();
        assert_eq!(giant_card.deploy_cost, 10);
        assert_eq!(giant_card.moves.len(), 2);
    }
}
