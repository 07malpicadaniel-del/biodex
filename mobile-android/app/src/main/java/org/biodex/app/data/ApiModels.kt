package org.biodex.app.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class CaptureUploadRequest(
    @SerialName("user_id") val userId: String? = null,
    @SerialName("image_base64") val imageBase64: String,
    val metadata: CaptureMetadataDto,
)

@Serializable
data class CaptureMetadataDto(
    @SerialName("capture_id") val captureId: String,
    @SerialName("captured_at_utc_ms") val capturedAtUtcMs: Long,
    val source: String,
    val coordinates: GeoPointDto,
    val imu: ImuReadingDto,
    @SerialName("moire_score") val moireScore: Float,
    @SerialName("image_sha256_base64") val imageSha256Base64: String,
    @SerialName("device_public_key_base64") val devicePublicKeyBase64: String,
    @SerialName("signature_base64") val signatureBase64: String,
)

@Serializable
data class GeoPointDto(
    val latitude: Double,
    val longitude: Double,
    @SerialName("accuracy_meters") val accuracyMeters: Float,
)

@Serializable
data class ImuReadingDto(
    @SerialName("accel_xyz") val accelXyz: List<Float>,
    @SerialName("gyro_xyz") val gyroXyz: List<Float>,
)

@Serializable
data class TaxonIdDto(
    val source: String,
    val id: Long,
)

@Serializable
data class CaptureUploadResponse(
    val status: String,
    @SerialName("capture_record_id") val captureRecordId: String,
    @SerialName("capture_id") val captureId: String,
    val classification: GeoClassificationDto,
    val identification: VisionIdentificationDto,
    val card: SpeciesCardDto? = null,
    @SerialName("naturalista_url") val naturalistaUrl: String? = null,
    @SerialName("image_url") val imageUrl: String? = null,
    @SerialName("taxonomic_class") val taxonomicClass: String? = null,
    val habitat: String? = null,
    @SerialName("iucn_status") val iucnStatus: String? = null,
    @SerialName("average_mass_grams") val averageMassGrams: Long? = null,
)

@Serializable
data class GeoClassificationDto(
    val classification: String,
    @SerialName("zone_id") val zoneId: String? = null,
    @SerialName("zone_name") val zoneName: String? = null,
    @SerialName("zone_type") val zoneType: String? = null,
)

@Serializable
data class VisionIdentificationDto(
    @SerialName("taxon_id") val taxonId: TaxonIdDto,
    @SerialName("scientific_name") val scientificName: String,
    @SerialName("common_name") val commonName: String? = null,
    val rank: String? = null,
    @SerialName("subspecies_label") val subspeciesLabel: String? = null,
    val confidence: Float,
)

@Serializable
data class SpeciesCardDto(
    @SerialName("taxon_id") val taxonId: TaxonIdDto,
    @SerialName("scientific_name") val scientificName: String,
    @SerialName("display_name") val displayName: String,
    val affinity: String,
    val rarity: String,
    val variant: String,
    @SerialName("deploy_cost") val deployCost: Int,
    val stats: CardStatsDto,
    val moves: List<CardMoveDto>,
    @SerialName("art_override") val artOverride: String? = null,
    @SerialName("applied_pack") val appliedPack: String,
)

@Serializable
data class CardStatsDto(
    val hp: Int,
    val attack: Int,
    val defense: Int,
    val speed: Int,
)

@Serializable
data class CardMoveDto(
    val id: String,
    val name: String,
    @SerialName("energy_cost") val energyCost: Int,
    val power: Int,
    val tag: String? = null,
)

@Serializable
data class AlmanacListResponse(
    @SerialName("total_species") val totalSpecies: Int,
    @SerialName("discovered_count") val discoveredCount: Int,
    @SerialName("matched_count") val matchedCount: Int = 0,
    @SerialName("category_counts") val categoryCounts: Map<String, Int> = emptyMap(),
    val entries: List<AlmanacItemDto>,
)

@Serializable
data class NaturaListaSyncResponse(
    val page: Int,
    @SerialName("fetched_candidates") val fetchedCandidates: Int,
    @SerialName("filtered_non_species") val filteredNonSpecies: Int,
    @SerialName("upserted_species") val upsertedSpecies: Int,
    @SerialName("total_catalog_species") val totalCatalogSpecies: Long,
)

@Serializable
data class AlmanacItemDto(
    @SerialName("taxon_id") val taxonId: TaxonIdDto,
    @SerialName("scientific_name") val scientificName: String,
    @SerialName("common_name") val commonName: String? = null,
    val phylum: String,
    @SerialName("taxonomic_class") val taxonomicClass: String,
    @SerialName("average_mass_grams") val averageMassGrams: Long,
    val diet: String,
    val habitat: String,
    val defenses: List<String> = emptyList(),
    @SerialName("iucn_status") val iucnStatus: String,
    val category: String,
    val summary: String,
    @SerialName("region_hint") val regionHint: String? = null,
    @SerialName("base_card") val baseCard: SpeciesCardDto? = null,
    val discovered: Boolean,
    @SerialName("best_variant") val bestVariant: String? = null,
    @SerialName("capture_count") val captureCount: Long = 0L,
    @SerialName("image_url") val imageUrl: String? = null,
    @SerialName("photo_attribution") val photoAttribution: String? = null,
    @SerialName("naturalista_url") val naturalistaUrl: String = "",
    @SerialName("wikipedia_url") val wikipediaUrl: String? = null,
    @SerialName("observations_count") val observationsCount: Long? = null,
    @SerialName("is_core_almanac") val isCoreAlmanac: Boolean = true,
    @SerialName("last_captured_at_utc") val lastCapturedAtUtc: String? = null,
    @SerialName("last_zone_name") val lastZoneName: String? = null,
)

@Serializable
data class NaturaListaSpeciesDto(
    @SerialName("taxon_id") val taxonId: TaxonIdDto,
    @SerialName("scientific_name") val scientificName: String,
    @SerialName("common_name") val commonName: String? = null,
    @SerialName("iconic_taxon_name") val iconicTaxonName: String? = null,
    @SerialName("iucn_status") val iucnStatus: String = "UNKNOWN",
    @SerialName("image_url") val imageUrl: String? = null,
    @SerialName("photo_attribution") val photoAttribution: String? = null,
    @SerialName("naturalista_url") val naturalistaUrl: String,
    @SerialName("wikipedia_url") val wikipediaUrl: String? = null,
    @SerialName("global_observations_count") val globalObservationsCount: Long? = null,
    @SerialName("local_observations_count") val localObservationsCount: Long? = null,
    val discovered: Boolean = false,
    @SerialName("best_variant") val bestVariant: String? = null,
    @SerialName("in_catalog") val inCatalog: Boolean = false,
)

@Serializable
data class NaturaListaDiscoveryResponse(
    @SerialName("source_url") val sourceUrl: String,
    @SerialName("query_summary") val querySummary: String,
    val entries: List<NaturaListaSpeciesDto>,
)

@Serializable
data class HealthResponse(
    val status: String,
    val database: String,
    @SerialName("postgis_version") val postgisVersion: String,
)

@Serializable
data class ApiErrorEnvelope(
    val error: ApiErrorBody,
)

@Serializable
data class ApiErrorBody(
    val code: String,
    val message: String,
)
