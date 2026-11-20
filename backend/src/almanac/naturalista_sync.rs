//! Live species synchronization from `.env`'s connected iNaturalist / NaturaLista API,
//! strictly excluding subspecies, varieties, hybrids, extinct taxa, and non-fauna entries.

use std::collections::BTreeSet;
use std::time::Duration;

use biodex_core::taxonomy::{Diet, Habitat, TaxonId};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::almanac::AlmanacError;
use crate::geo::taxon_db_parts;
use crate::vision::{
    INatTaxaResponse, INatTaxon, TaxonomicRank, VisionError, clean_opt, extract_inat_media,
    infer_defaults_from_taxonomy, is_non_fauna_taxon, parse_iucn, serialize_slug,
};

/// Verified iNaturalist ancestor taxon identifiers mapped into BioDex Collection clades.
const INAT_CANIDAE: u64 = 42_043;
const INAT_FELIDAE: u64 = 41_944;
const INAT_PRIMATES: u64 = 43_367;
const INAT_ELASMOBRANCHII: u64 = 47_273;
const INAT_CARCHARHINIFORMES: u64 = 48_069;
const INAT_LAMNIFORMES: u64 = 50_870;
const INAT_MYLIOBATIFORMES: u64 = 196_615;
const INAT_CETACEA: u64 = 152_871;
const INAT_DELPHINIDAE: u64 = 41_479;
const INAT_BALAENOPTERIDAE: u64 = 41_546;
const INAT_OTARIIDAE: u64 = 41_736;
const INAT_PHOCIDAE: u64 = 41_687;
const INAT_CHELONIOIDEA: u64 = 372_234;
const INAT_SYNGNATHIDAE: u64 = 49_106;
const INAT_CEPHALOPODA: u64 = 47_459;
const INAT_MOLLUSCA: u64 = 47_115;
const INAT_ARACHNIDA: u64 = 47_119;
const INAT_MALACOSTRACA: u64 = 47_187;
const INAT_CNIDARIA: u64 = 47_534;
const INAT_ECHINODERMATA: u64 = 47_549;
const INAT_INSECTA: u64 = 47_158;
const INAT_ACTINOPTERYGII: u64 = 47_178;
const INAT_MAMMALIA: u64 = 40_151;
const INAT_AVES: u64 = 3;
const INAT_REPTILIA: u64 = 26_036;
const INAT_AMPHIBIA: u64 = 20_978;

// Charismatic orders and families so iconic wildlife (Harpy Eagle, Komodo Dragon, Blue-tongued Skinks, etc.)
// is guaranteed in the backend index without getting buried in mega-classes like Aves or Insecta.
const INAT_ACCIPITRIFORMES: u64 = 71_261;
const INAT_FALCONIFORMES: u64 = 67_570;
const INAT_STRIGIFORMES: u64 = 19_350;
const INAT_PSITTACIFORMES: u64 = 18_874;
const INAT_SPHENISCIFORMES: u64 = 67_564;
const INAT_RAMPHASTIDAE: u64 = 18_485;
const INAT_TROCHILIDAE: u64 = 5_562;
const INAT_PHOENICOPTERIFORMES: u64 = 67_569;
const INAT_VARANIDAE: u64 = 39_392;
const INAT_SCINCIDAE: u64 = 36_982;
const INAT_CHAMAELEONIDAE: u64 = 32_805;
const INAT_IGUANIDAE: u64 = 35_278;
const INAT_CROCODYLIA: u64 = 26_039;
const INAT_TESTUDINES: u64 = 39_532;
const INAT_SERPENTES: u64 = 85_553;
const INAT_CAUDATA: u64 = 26_718;
const INAT_ANURA: u64 = 20_979;
const INAT_URSIDAE: u64 = 41_636;
const INAT_PROBOSCIDEA: u64 = 43_691;
const INAT_RHINOCEROTIDAE: u64 = 43_341;
const INAT_HIPPOPOTAMIDAE: u64 = 42_146;
const INAT_GIRAFFIDAE: u64 = 42_153;
const INAT_CERVIDAE: u64 = 42_158;
const INAT_BOVIDAE: u64 = 42_241;
const INAT_CAMELIDAE: u64 = 42_231;
const INAT_EQUIDAE: u64 = 43_328;
const INAT_SUIDAE: u64 = 42_118;
const INAT_MUSTELIDAE: u64 = 41_770;
const INAT_PROCYONIDAE: u64 = 41_660;
const INAT_HYAENIDAE: u64 = 41_884;
const INAT_XENARTHRA: u64 = 848_322;
const INAT_DIPROTODONTIA: u64 = 42_711;
const INAT_DIDELPHIMORPHIA: u64 = 42_540;
const INAT_CHIROPTERA: u64 = 40_268;
const INAT_RODENTIA: u64 = 43_698;

/// Clade sync target paired with a human-readable Spanish description fallback.
#[derive(Debug, Clone, Copy)]
struct CladeTarget {
    taxon_id: u64,
    default_category: &'static str,
    group_label_es: &'static str,
}

const SYNC_CLADES: &[CladeTarget] = &[
    // --- Requested & charismatic focal clades first ---
    CladeTarget {
        taxon_id: INAT_CANIDAE,
        default_category: "canids",
        group_label_es: "Cánido silvestre o doméstico",
    },
    CladeTarget {
        taxon_id: INAT_FELIDAE,
        default_category: "felines",
        group_label_es: "Felino",
    },
    CladeTarget {
        taxon_id: INAT_PRIMATES,
        default_category: "primates",
        group_label_es: "Primate",
    },
    CladeTarget {
        taxon_id: INAT_ELASMOBRANCHII,
        default_category: "sharks",
        group_label_es: "Escuálido marino (tiburón o raya)",
    },
    CladeTarget {
        taxon_id: INAT_CARCHARHINIFORMES,
        default_category: "sharks",
        group_label_es: "Tiburón carcariniforme",
    },
    CladeTarget {
        taxon_id: INAT_LAMNIFORMES,
        default_category: "sharks",
        group_label_es: "Tiburón lámnido",
    },
    CladeTarget {
        taxon_id: INAT_MYLIOBATIFORMES,
        default_category: "sharks",
        group_label_es: "Raya o mantarraya",
    },
    // --- Iconic Birds (Águila arpía, halcones, búhos, guacamayas, pingüinos, tucanes, colibríes) ---
    CladeTarget {
        taxon_id: INAT_ACCIPITRIFORMES,
        default_category: "birds",
        group_label_es: "Ave rapaz (águilas y gavilanes)",
    },
    CladeTarget {
        taxon_id: INAT_FALCONIFORMES,
        default_category: "birds",
        group_label_es: "Halcón o caracara",
    },
    CladeTarget {
        taxon_id: INAT_STRIGIFORMES,
        default_category: "birds",
        group_label_es: "Ave rapaz nocturna (búho o lechuza)",
    },
    CladeTarget {
        taxon_id: INAT_PSITTACIFORMES,
        default_category: "birds",
        group_label_es: "Psitácido (guacamaya, loro o perico)",
    },
    CladeTarget {
        taxon_id: INAT_SPHENISCIFORMES,
        default_category: "birds",
        group_label_es: "Pingüino marino",
    },
    CladeTarget {
        taxon_id: INAT_RAMPHASTIDAE,
        default_category: "birds",
        group_label_es: "Tucán o aracari",
    },
    CladeTarget {
        taxon_id: INAT_TROCHILIDAE,
        default_category: "birds",
        group_label_es: "Colibrí",
    },
    CladeTarget {
        taxon_id: INAT_PHOENICOPTERIFORMES,
        default_category: "birds",
        group_label_es: "Flamenco",
    },
    // --- Iconic Reptiles & Amphibians (Dragón de Komodo, Escincos lengua azul, Camaleones, etc.) ---
    CladeTarget {
        taxon_id: INAT_VARANIDAE,
        default_category: "reptiles_amphibians",
        group_label_es: "Varano (monitor o dragón de Komodo)",
    },
    CladeTarget {
        taxon_id: INAT_SCINCIDAE,
        default_category: "reptiles_amphibians",
        group_label_es: "Escinco (lagarto de lengua azul y afines)",
    },
    CladeTarget {
        taxon_id: INAT_CHAMAELEONIDAE,
        default_category: "reptiles_amphibians",
        group_label_es: "Camaleón",
    },
    CladeTarget {
        taxon_id: INAT_IGUANIDAE,
        default_category: "reptiles_amphibians",
        group_label_es: "Iguana",
    },
    CladeTarget {
        taxon_id: INAT_CROCODYLIA,
        default_category: "reptiles_amphibians",
        group_label_es: "Cocodrilo, caimán o gavial",
    },
    CladeTarget {
        taxon_id: INAT_TESTUDINES,
        default_category: "reptiles_amphibians",
        group_label_es: "Tortuga terrestre o dulceacuícola",
    },
    CladeTarget {
        taxon_id: INAT_SERPENTES,
        default_category: "reptiles_amphibians",
        group_label_es: "Serpiente",
    },
    CladeTarget {
        taxon_id: INAT_CAUDATA,
        default_category: "reptiles_amphibians",
        group_label_es: "Salamandra, ajolote o tritón",
    },
    CladeTarget {
        taxon_id: INAT_ANURA,
        default_category: "reptiles_amphibians",
        group_label_es: "Rana o sapo",
    },
    // --- Iconic Mammals (Osos, elefantes, rinocerontes, jirafas, marsupiales, etc.) ---
    CladeTarget {
        taxon_id: INAT_URSIDAE,
        default_category: "mammals",
        group_label_es: "Úrsido (oso)",
    },
    CladeTarget {
        taxon_id: INAT_PROBOSCIDEA,
        default_category: "mammals",
        group_label_es: "Elefante",
    },
    CladeTarget {
        taxon_id: INAT_RHINOCEROTIDAE,
        default_category: "mammals",
        group_label_es: "Rinoceronte",
    },
    CladeTarget {
        taxon_id: INAT_HIPPOPOTAMIDAE,
        default_category: "mammals",
        group_label_es: "Hipopótamo",
    },
    CladeTarget {
        taxon_id: INAT_GIRAFFIDAE,
        default_category: "mammals",
        group_label_es: "Jirafa u okapi",
    },
    CladeTarget {
        taxon_id: INAT_CERVIDAE,
        default_category: "mammals",
        group_label_es: "Cérvido (venado o alce)",
    },
    CladeTarget {
        taxon_id: INAT_BOVIDAE,
        default_category: "mammals",
        group_label_es: "Bóvido o antílope",
    },
    CladeTarget {
        taxon_id: INAT_CAMELIDAE,
        default_category: "mammals",
        group_label_es: "Camélido",
    },
    CladeTarget {
        taxon_id: INAT_EQUIDAE,
        default_category: "mammals",
        group_label_es: "Équido (cebra o asno silvestre)",
    },
    CladeTarget {
        taxon_id: INAT_SUIDAE,
        default_category: "mammals",
        group_label_es: "Súido (jabalí o facóquero)",
    },
    CladeTarget {
        taxon_id: INAT_MUSTELIDAE,
        default_category: "mammals",
        group_label_es: "Mustélido (nutria, tejón o comadreja)",
    },
    CladeTarget {
        taxon_id: INAT_PROCYONIDAE,
        default_category: "mammals",
        group_label_es: "Prociónido (mapache, coatí o cacomixtle)",
    },
    CladeTarget {
        taxon_id: INAT_HYAENIDAE,
        default_category: "mammals",
        group_label_es: "Hiénido",
    },
    CladeTarget {
        taxon_id: INAT_XENARTHRA,
        default_category: "mammals",
        group_label_es: "Xenartro (perezoso, armadillo u oso hormiguero)",
    },
    CladeTarget {
        taxon_id: INAT_DIPROTODONTIA,
        default_category: "mammals",
        group_label_es: "Marsupial australiano (canguro, koala o wombat)",
    },
    CladeTarget {
        taxon_id: INAT_DIDELPHIMORPHIA,
        default_category: "mammals",
        group_label_es: "Tlacuache o zarigüeya",
    },
    CladeTarget {
        taxon_id: INAT_CHIROPTERA,
        default_category: "mammals",
        group_label_es: "Murciélago",
    },
    CladeTarget {
        taxon_id: INAT_RODENTIA,
        default_category: "mammals",
        group_label_es: "Roedor (capibara, castor o ardilla)",
    },
    // --- Aquatic Mammals, Sea Turtles, Seahorses & Bony Fish ---
    CladeTarget {
        taxon_id: INAT_CETACEA,
        default_category: "aquatic",
        group_label_es: "Cetáceo marino",
    },
    CladeTarget {
        taxon_id: INAT_DELPHINIDAE,
        default_category: "aquatic",
        group_label_es: "Delfínido marino",
    },
    CladeTarget {
        taxon_id: INAT_BALAENOPTERIDAE,
        default_category: "aquatic",
        group_label_es: "Ballena rorcual",
    },
    CladeTarget {
        taxon_id: INAT_OTARIIDAE,
        default_category: "aquatic",
        group_label_es: "Lobo o león marino",
    },
    CladeTarget {
        taxon_id: INAT_PHOCIDAE,
        default_category: "aquatic",
        group_label_es: "Foca marina",
    },
    CladeTarget {
        taxon_id: INAT_CHELONIOIDEA,
        default_category: "aquatic",
        group_label_es: "Tortuga marina",
    },
    CladeTarget {
        taxon_id: INAT_SYNGNATHIDAE,
        default_category: "aquatic",
        group_label_es: "Caballito de mar o pez pipa",
    },
    CladeTarget {
        taxon_id: INAT_ACTINOPTERYGII,
        default_category: "aquatic",
        group_label_es: "Pez óseo acuático",
    },
    // --- Invertebrates & Insects ---
    CladeTarget {
        taxon_id: INAT_CEPHALOPODA,
        default_category: "invertebrates",
        group_label_es: "Cefalópodo marino (pulpo, calamar o sepia)",
    },
    CladeTarget {
        taxon_id: INAT_MOLLUSCA,
        default_category: "invertebrates",
        group_label_es: "Molusco invertebrado",
    },
    CladeTarget {
        taxon_id: INAT_ARACHNIDA,
        default_category: "invertebrates",
        group_label_es: "Arácnido invertebrado",
    },
    CladeTarget {
        taxon_id: INAT_MALACOSTRACA,
        default_category: "invertebrates",
        group_label_es: "Crustáceo decápodo o malacostraco",
    },
    CladeTarget {
        taxon_id: INAT_CNIDARIA,
        default_category: "invertebrates",
        group_label_es: "Cnidario marino",
    },
    CladeTarget {
        taxon_id: INAT_ECHINODERMATA,
        default_category: "invertebrates",
        group_label_es: "Equinodermo marino",
    },
    CladeTarget {
        taxon_id: INAT_INSECTA,
        default_category: "insects_arachnids",
        group_label_es: "Insecto",
    },
    // --- Broad superclass catch-alls ---
    CladeTarget {
        taxon_id: INAT_MAMMALIA,
        default_category: "mammals",
        group_label_es: "Mamífero",
    },
    CladeTarget {
        taxon_id: INAT_AVES,
        default_category: "birds",
        group_label_es: "Ave",
    },
    CladeTarget {
        taxon_id: INAT_REPTILIA,
        default_category: "reptiles_amphibians",
        group_label_es: "Reptil",
    },
    CladeTarget {
        taxon_id: INAT_AMPHIBIA,
        default_category: "reptiles_amphibians",
        group_label_es: "Anfibio",
    },
];

/// Result summary returned after syncing species from NaturaLista into PostgreSQL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NaturaListaSyncReport {
    /// API page number fetched for each clade (`1..`).
    pub page: u32,
    /// Total candidates parsed from iNaturalist responses.
    pub fetched_candidates: usize,
    /// Subspecies, hybrids, extinct taxa, or non-fauna entries discarded by validation.
    pub filtered_non_species: usize,
    /// Valid living animal species upserted into `species_catalog`.
    pub upserted_species: usize,
    /// Total active Collection species visible in the UI catalog after the sync.
    pub total_catalog_species: i64,
    /// Total living animal species indexed on the backend for Scanner & search resolution.
    #[serde(default)]
    pub total_backend_indexed: i64,
}

/// Optional filter when triggering sync (`page` defaults to `1`; `category` limits to one group).
#[derive(Debug, Default, Clone, Deserialize)]
pub struct NaturaListaSyncOptions {
    /// 1-based page per clade (`1` by default, clamped to `1..=50`).
    pub page: Option<u32>,
    /// Optional single category filter (e.g., `"canids"`, `"felines"`, `"sharks"`).
    pub category: Option<String>,
    /// When `true`, indexes new species into the backend (`is_backend_indexed = TRUE`) without
    /// bloating the UI default browse list (`is_naturalista_synced` remains unchanged on existing rows).
    pub backend_only: Option<bool>,
}

/// Validates that an iNaturalist taxon is strictly an active, living, binomial `species`
/// belonging to non-human fauna and carrying a photograph URL.
#[must_use]
pub(crate) fn is_valid_living_species_taxon(taxon: &INatTaxon) -> bool {
    if taxon.is_active == Some(false) || taxon.extinct == Some(true) {
        return false;
    }
    if is_non_fauna_taxon(taxon) {
        return false;
    }

    let rank = TaxonomicRank::from_inaturalist(taxon.rank.as_deref(), taxon.rank_level);
    if rank != TaxonomicRank::Species || taxon.rank_level != Some(10) {
        return false;
    }

    let trimmed_name = taxon.name.trim();
    if trimmed_name.contains('×') || trimmed_name.split_whitespace().count() != 2 {
        return false;
    }

    extract_inat_media(taxon).image_url.is_some()
}

/// Maps an iNaturalist taxon's `ancestor_ids` and iconic taxon into its exact BioDex Collection clade.
#[must_use]
pub(crate) fn classify_inat_taxon_clade(
    taxon: &INatTaxon,
    fallback_category: &str,
) -> (&'static str, &'static str, &'static str) {
    let has = |id: u64| taxon.id == id || taxon.ancestor_ids.contains(&id);

    if has(INAT_CANIDAE) {
        return ("canids", "Chordata", "Mammalia");
    }
    if has(INAT_FELIDAE) {
        return ("felines", "Chordata", "Mammalia");
    }
    if has(INAT_PRIMATES) {
        return ("primates", "Chordata", "Mammalia");
    }
    if has(INAT_ELASMOBRANCHII)
        || has(INAT_CARCHARHINIFORMES)
        || has(INAT_LAMNIFORMES)
        || has(INAT_MYLIOBATIFORMES)
    {
        return ("sharks", "Chordata", "Chondrichthyes");
    }
    if has(INAT_CETACEA)
        || has(INAT_DELPHINIDAE)
        || has(INAT_BALAENOPTERIDAE)
        || has(INAT_OTARIIDAE)
        || has(INAT_PHOCIDAE)
    {
        return ("aquatic", "Chordata", "Mammalia");
    }
    if has(INAT_CHELONIOIDEA) {
        return ("aquatic", "Chordata", "Reptilia");
    }
    if has(INAT_CEPHALOPODA) {
        return ("invertebrates", "Mollusca", "Cephalopoda");
    }
    if has(INAT_MOLLUSCA) {
        return ("invertebrates", "Mollusca", "Mollusca");
    }
    if has(INAT_ARACHNIDA) {
        return ("invertebrates", "Arthropoda", "Arachnida");
    }
    if has(INAT_MALACOSTRACA) {
        return ("invertebrates", "Arthropoda", "Malacostraca");
    }
    if has(INAT_CNIDARIA) {
        return ("invertebrates", "Cnidaria", "Cnidaria");
    }
    if has(INAT_ECHINODERMATA) {
        return ("invertebrates", "Echinodermata", "Echinodermata");
    }
    if has(INAT_INSECTA) {
        return ("insects_arachnids", "Arthropoda", "Insecta");
    }
    if has(INAT_SYNGNATHIDAE) || has(INAT_ACTINOPTERYGII) {
        return ("aquatic", "Chordata", "Actinopterygii");
    }
    if has(INAT_ACCIPITRIFORMES)
        || has(INAT_FALCONIFORMES)
        || has(INAT_STRIGIFORMES)
        || has(INAT_PSITTACIFORMES)
        || has(INAT_SPHENISCIFORMES)
        || has(INAT_RAMPHASTIDAE)
        || has(INAT_TROCHILIDAE)
        || has(INAT_PHOENICOPTERIFORMES)
        || has(INAT_AVES)
    {
        return ("birds", "Chordata", "Aves");
    }
    if has(INAT_CAUDATA) || has(INAT_ANURA) || has(INAT_AMPHIBIA) {
        return ("reptiles_amphibians", "Chordata", "Amphibia");
    }
    if has(INAT_VARANIDAE)
        || has(INAT_SCINCIDAE)
        || has(INAT_CHAMAELEONIDAE)
        || has(INAT_IGUANIDAE)
        || has(INAT_CROCODYLIA)
        || has(INAT_TESTUDINES)
        || has(INAT_SERPENTES)
        || has(INAT_REPTILIA)
    {
        return ("reptiles_amphibians", "Chordata", "Reptilia");
    }
    if has(INAT_URSIDAE)
        || has(INAT_PROBOSCIDEA)
        || has(INAT_RHINOCEROTIDAE)
        || has(INAT_HIPPOPOTAMIDAE)
        || has(INAT_GIRAFFIDAE)
        || has(INAT_CERVIDAE)
        || has(INAT_BOVIDAE)
        || has(INAT_CAMELIDAE)
        || has(INAT_EQUIDAE)
        || has(INAT_SUIDAE)
        || has(INAT_MUSTELIDAE)
        || has(INAT_PROCYONIDAE)
        || has(INAT_HYAENIDAE)
        || has(INAT_XENARTHRA)
        || has(INAT_DIPROTODONTIA)
        || has(INAT_DIDELPHIMORPHIA)
        || has(INAT_CHIROPTERA)
        || has(INAT_RODENTIA)
        || has(INAT_MAMMALIA)
    {
        return ("mammals", "Chordata", "Mammalia");
    }

    let iconic = taxon.iconic_taxon_name.as_deref().unwrap_or("");
    match iconic {
        "Aves" => ("birds", "Chordata", "Aves"),
        "Mammalia" => ("mammals", "Chordata", "Mammalia"),
        "Reptilia" => ("reptiles_amphibians", "Chordata", "Reptilia"),
        "Amphibia" => ("reptiles_amphibians", "Chordata", "Amphibia"),
        "Insecta" => ("insects_arachnids", "Arthropoda", "Insecta"),
        "Arachnida" => ("invertebrates", "Arthropoda", "Arachnida"),
        "Mollusca" => ("invertebrates", "Mollusca", "Mollusca"),
        "Actinopterygii" => ("aquatic", "Chordata", "Actinopterygii"),
        _ => match fallback_category {
            "sharks" => ("sharks", "Chordata", "Chondrichthyes"),
            "invertebrates" => ("invertebrates", "Animalia", "Invertebrata"),
            "canids" => ("canids", "Chordata", "Mammalia"),
            "felines" => ("felines", "Chordata", "Mammalia"),
            "primates" => ("primates", "Chordata", "Mammalia"),
            "birds" => ("birds", "Chordata", "Aves"),
            "reptiles_amphibians" => ("reptiles_amphibians", "Chordata", "Reptilia"),
            "insects_arachnids" => ("insects_arachnids", "Arthropoda", "Insecta"),
            "mammals" => ("mammals", "Chordata", "Mammalia"),
            _ => ("aquatic", "Chordata", "Actinopterygii"),
        },
    }
}

/// Parses an iNaturalist `/v1/taxa` payload, keeping strictly valid living binomial species.
pub(crate) fn extract_valid_species_from_taxa_json(
    raw_json: &str,
) -> Result<(Vec<INatTaxon>, usize), AlmanacError> {
    let parsed: INatTaxaResponse = serde_json::from_str(raw_json)
        .map_err(|err| AlmanacError::Vision(VisionError::Upstream(err.to_string())))?;

    let total_raw = parsed.results.len();
    let valid: Vec<INatTaxon> = parsed
        .results
        .into_iter()
        .filter(is_valid_living_species_taxon)
        .collect();
    let filtered = total_raw.saturating_sub(valid.len());
    Ok((valid, filtered))
}

/// Synchronizes verified species from `.env`'s connected iNaturalist / NaturaLista endpoint
/// into `species_catalog` (`taxonomic_rank = 'species'`).
pub async fn sync_naturalista_clade_species(
    pool: &PgPool,
    http: &reqwest::Client,
    inaturalist_base_url: &str,
    api_token: Option<&str>,
    options: &NaturaListaSyncOptions,
) -> Result<NaturaListaSyncReport, AlmanacError> {
    let page = options.page.unwrap_or(1).clamp(1, 50);
    let page_str = page.to_string();
    let backend_only = options.backend_only.unwrap_or(false);
    let category_filter = options
        .category
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_ascii_lowercase);

    let targets: Vec<&CladeTarget> = SYNC_CLADES
        .iter()
        .filter(|t| {
            category_filter
                .as_deref()
                .is_none_or(|wanted| t.default_category == wanted)
        })
        .collect();

    if targets.is_empty() {
        return Err(AlmanacError::InvalidInput(
            "categoría desconocida para sincronizar con NaturaLista".to_string(),
        ));
    }

    let base = inaturalist_base_url.trim_end_matches('/');
    let endpoint = format!("{base}/taxa");

    let mut fetched_candidates = 0usize;
    let mut filtered_non_species = 0usize;
    let mut upserted_species = 0usize;
    let mut seen_ids = BTreeSet::new();

    for (idx, target) in targets.iter().enumerate() {
        if idx > 0 {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }

        let taxon_param = target.taxon_id.to_string();
        let build_req = || {
            let mut req = http.get(&endpoint).query(&[
                ("taxon_id", taxon_param.as_str()),
                ("rank", "species"),
                ("is_active", "true"),
                ("order", "desc"),
                ("order_by", "observations_count"),
                ("locale", "es"),
                ("per_page", "200"),
                ("page", page_str.as_str()),
            ]);
            if let Some(token) = api_token
                && !token.trim().is_empty()
            {
                req = req.bearer_auth(token.trim());
            }
            req
        };

        let mut resp = match build_req().send().await {
            Ok(r) => r,
            Err(err) => {
                tracing::warn!(taxon_id = target.taxon_id, error = %err, "NaturaLista clade sync HTTP error");
                continue;
            }
        };

        if resp.status().as_u16() == 429 {
            tracing::warn!(
                taxon_id = target.taxon_id,
                "NaturaLista rate limit hit (429), retrying after pause"
            );
            tokio::time::sleep(Duration::from_secs(2)).await;
            resp = match build_req().send().await {
                Ok(r) => r,
                Err(err) => {
                    tracing::warn!(taxon_id = target.taxon_id, error = %err, "NaturaLista clade sync HTTP retry error");
                    continue;
                }
            };
        }

        if !resp.status().is_success() {
            tracing::warn!(
                taxon_id = target.taxon_id,
                status = %resp.status(),
                "NaturaLista clade sync returned non-success"
            );
            continue;
        }

        let body = resp
            .text()
            .await
            .map_err(|err| AlmanacError::Vision(VisionError::Upstream(err.to_string())))?;

        let (valid_taxa, filtered_count) = extract_valid_species_from_taxa_json(&body)?;
        fetched_candidates += valid_taxa.len() + filtered_count;
        filtered_non_species += filtered_count;

        for taxon in valid_taxa {
            if !seen_ids.insert(taxon.id) {
                continue;
            }
            if upsert_synced_inat_taxon(pool, &taxon, target, !backend_only).await? {
                upserted_species += 1;
            }
        }
    }

    let total_catalog_species: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM species_catalog WHERE is_core_almanac = TRUE OR is_naturalista_synced = TRUE",
    )
    .fetch_one(pool)
    .await?;

    let total_backend_indexed: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM species_catalog WHERE is_core_almanac = TRUE OR is_naturalista_synced = TRUE OR is_backend_indexed = TRUE",
    )
    .fetch_one(pool)
    .await?;

    Ok(NaturaListaSyncReport {
        page,
        fetched_candidates,
        filtered_non_species,
        upserted_species,
        total_catalog_species,
        total_backend_indexed,
    })
}

async fn upsert_synced_inat_taxon(
    pool: &PgPool,
    taxon: &INatTaxon,
    target: &CladeTarget,
    visible_in_collection: bool,
) -> Result<bool, AlmanacError> {
    let taxon_id = TaxonId::INaturalist(taxon.id);
    let (taxon_source, taxon_external_id) = taxon_db_parts(taxon_id)?;
    let (category, phylum, taxonomic_class) =
        classify_inat_taxon_clade(taxon, target.default_category);

    let inferred = infer_defaults_from_taxonomy(phylum, taxonomic_class, None, None);
    let habitat = if category == "sharks"
        || (category == "aquatic" && inferred.habitat == Habitat::Terrestrial)
    {
        Habitat::Marine
    } else {
        inferred.habitat
    };
    let diet = match category {
        "felines" | "sharks" => Diet::Carnivore,
        "primates" => Diet::Omnivore,
        _ => inferred.diet,
    };
    let mass_g: u64 = match category {
        "canids" => 12_000,
        "felines" => 25_000,
        "primates" => 12_000,
        "sharks" => 90_000,
        "aquatic" if taxonomic_class == "Mammalia" => 400_000,
        _ => inferred.average_mass_grams,
    };

    let iucn = taxon
        .conservation_status
        .as_ref()
        .and_then(|cs| cs.status.as_deref())
        .map(parse_iucn)
        .unwrap_or(biodex_core::taxonomy::IucnStatus::Unknown);

    let media = extract_inat_media(taxon);
    let common_name = clean_opt(taxon.preferred_common_name.as_deref());
    let summary = build_naturalista_summary(taxon, target, common_name.as_deref());

    let diet_slug = serialize_slug(&diet);
    let habitat_slug = serialize_slug(&habitat);
    let iucn_slug = serialize_slug(&iucn);
    let defenses_json =
        serde_json::to_value(&inferred.defenses).unwrap_or(serde_json::Value::Array(vec![]));
    let mass_i64 = i64::try_from(mass_g.max(1)).unwrap_or(1_000);

    sqlx::query(
        r#"
        INSERT INTO species_catalog (
            taxon_source, taxon_external_id, scientific_name, common_name,
            phylum, taxonomic_class, average_mass_grams, diet, habitat, defenses,
            iucn_status, summary, category, is_core_almanac, is_naturalista_synced,
            is_backend_indexed, taxonomic_rank, image_url, photo_attribution, wikipedia_url,
            naturalista_observations_count
        )
        VALUES (
            $1, $2, $3, $4,
            $5, $6, $7, $8, $9, $10,
            $11, $12, $13, FALSE, $18,
            TRUE, 'species', $14, $15, $16,
            $17
        )
        ON CONFLICT (taxon_source, taxon_external_id) DO UPDATE SET
            common_name = CASE
                WHEN species_catalog.is_core_almanac THEN species_catalog.common_name
                ELSE COALESCE(EXCLUDED.common_name, species_catalog.common_name)
            END,
            category = CASE
                WHEN species_catalog.is_core_almanac THEN species_catalog.category
                ELSE EXCLUDED.category
            END,
            iucn_status = CASE
                WHEN species_catalog.iucn_status <> 'UNKNOWN' THEN species_catalog.iucn_status
                ELSE EXCLUDED.iucn_status
            END,
            is_naturalista_synced = species_catalog.is_naturalista_synced OR EXCLUDED.is_naturalista_synced,
            is_backend_indexed = TRUE,
            taxonomic_rank = 'species',
            image_url = COALESCE(species_catalog.image_url, EXCLUDED.image_url),
            photo_attribution = COALESCE(species_catalog.photo_attribution, EXCLUDED.photo_attribution),
            wikipedia_url = COALESCE(species_catalog.wikipedia_url, EXCLUDED.wikipedia_url),
            naturalista_observations_count = COALESCE(
                EXCLUDED.naturalista_observations_count,
                species_catalog.naturalista_observations_count
            )
        "#,
    )
    .bind(taxon_source)
    .bind(taxon_external_id)
    .bind(taxon.name.trim())
    .bind(&common_name)
    .bind(phylum)
    .bind(taxonomic_class)
    .bind(mass_i64)
    .bind(diet_slug)
    .bind(habitat_slug)
    .bind(defenses_json)
    .bind(iucn_slug)
    .bind(summary)
    .bind(category)
    .bind(&media.image_url)
    .bind(&media.photo_attribution)
    .bind(&media.wikipedia_url)
    .bind(media.observations_count)
    .bind(visible_in_collection)
    .execute(pool)
    .await?;

    Ok(true)
}

fn build_naturalista_summary(
    taxon: &INatTaxon,
    target: &CladeTarget,
    common_name: Option<&str>,
) -> String {
    let label = common_name.unwrap_or(taxon.name.trim());
    match taxon.observations_count {
        Some(obs) if obs > 0 => format!(
            "{} ({}): {} registrado en NaturaLista con {} observaciones verificadas.",
            label,
            taxon.name.trim(),
            target.group_label_es,
            obs
        ),
        _ => format!(
            "{} ({}): {} catalogado directamente desde NaturaLista.",
            label,
            taxon.name.trim(),
            target.group_label_es
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_out_subspecies_hybrids_extinct_and_homo_sapiens() {
        let payload = r#"{
            "results": [
                {
                    "id": 42051,
                    "name": "Canis latrans",
                    "rank": "species",
                    "rank_level": 10,
                    "is_active": true,
                    "extinct": false,
                    "preferred_common_name": "Coyote",
                    "iconic_taxon_name": "Mammalia",
                    "ancestor_ids": [1, 2, 40151, 42043],
                    "default_photo": { "medium_url": "https://example.org/coyote.jpg" }
                },
                {
                    "id": 130793,
                    "name": "Panthera tigris tigris",
                    "rank": "subspecies",
                    "rank_level": 5,
                    "is_active": true,
                    "extinct": false,
                    "iconic_taxon_name": "Mammalia",
                    "default_photo": { "medium_url": "https://example.org/sub.jpg" }
                },
                {
                    "id": 43584,
                    "name": "Homo sapiens",
                    "rank": "species",
                    "rank_level": 10,
                    "is_active": true,
                    "extinct": false,
                    "iconic_taxon_name": "Mammalia",
                    "default_photo": { "medium_url": "https://example.org/human.jpg" }
                },
                {
                    "id": 500001,
                    "name": "Smilodon fatalis",
                    "rank": "species",
                    "rank_level": 10,
                    "is_active": true,
                    "extinct": true,
                    "iconic_taxon_name": "Mammalia",
                    "default_photo": { "medium_url": "https://example.org/smilodon.jpg" }
                },
                {
                    "id": 500002,
                    "name": "Canis × latrans",
                    "rank": "species",
                    "rank_level": 10,
                    "is_active": true,
                    "extinct": false,
                    "iconic_taxon_name": "Mammalia",
                    "default_photo": { "medium_url": "https://example.org/hybrid.jpg" }
                }
            ]
        }"#;

        let (valid, filtered) = extract_valid_species_from_taxa_json(payload).unwrap();
        assert_eq!(valid.len(), 1);
        assert_eq!(filtered, 4);
        assert_eq!(valid[0].id, 42_051);

        let (cat, phylum, class_name) = classify_inat_taxon_clade(&valid[0], "mammals");
        assert_eq!(cat, "canids");
        assert_eq!(phylum, "Chordata");
        assert_eq!(class_name, "Mammalia");
    }

    #[test]
    fn disambiguates_cetaceans_and_sharks_from_mammals_and_fishes() {
        let orca: INatTaxon = serde_json::from_str(
            r#"{
                "id": 41521,
                "name": "Orcinus orca",
                "rank": "species",
                "rank_level": 10,
                "ancestor_ids": [1, 2, 40151, 152871]
            }"#,
        )
        .unwrap();
        let (cat, _, class_name) = classify_inat_taxon_clade(&orca, "mammals");
        assert_eq!(cat, "aquatic");
        assert_eq!(class_name, "Mammalia");

        let shark: INatTaxon = serde_json::from_str(
            r#"{
                "id": 50873,
                "name": "Carcharodon carcharias",
                "rank": "species",
                "rank_level": 10,
                "ancestor_ids": [1, 2, 47273]
            }"#,
        )
        .unwrap();
        let (cat_s, _, class_s) = classify_inat_taxon_clade(&shark, "aquatic");
        assert_eq!(cat_s, "sharks");
        assert_eq!(class_s, "Chondrichthyes");

        let komodo: INatTaxon = serde_json::from_str(
            r#"{
                "id": 39449,
                "name": "Varanus komodoensis",
                "rank": "species",
                "rank_level": 10,
                "ancestor_ids": [1, 2, 26036, 39392]
            }"#,
        )
        .unwrap();
        let (cat_k, _, class_k) = classify_inat_taxon_clade(&komodo, "reptiles_amphibians");
        assert_eq!(cat_k, "reptiles_amphibians");
        assert_eq!(class_k, "Reptilia");

        let harpy: INatTaxon = serde_json::from_str(
            r#"{
                "id": 5321,
                "name": "Harpia harpyja",
                "rank": "species",
                "rank_level": 10,
                "ancestor_ids": [1, 2, 3, 71261]
            }"#,
        )
        .unwrap();
        let (cat_h, _, class_h) = classify_inat_taxon_clade(&harpy, "birds");
        assert_eq!(cat_h, "birds");
        assert_eq!(class_h, "Aves");
    }
}
