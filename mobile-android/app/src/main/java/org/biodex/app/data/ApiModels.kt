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
    val entries: List<AlmanacItemDto>,
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
    @SerialName("base_card") val baseCard: SpeciesCardDto,
    val discovered: Boolean,
    @SerialName("best_variant") val bestVariant: String? = null,
    @SerialName("capture_count") val captureCount: Long = 0L,
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
