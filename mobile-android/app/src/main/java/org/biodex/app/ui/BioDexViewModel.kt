package org.biodex.app.ui

import android.app.Application
import android.content.Context
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.biodex.app.BuildConfig
import org.biodex.app.data.AlmanacItemDto
import org.biodex.app.data.BioDexApiClient
import org.biodex.app.data.CaptureMetadataDto
import org.biodex.app.data.CaptureUploadRequest
import org.biodex.app.data.CaptureUploadResponse
import org.biodex.app.data.GeoPointDto
import org.biodex.app.data.ImuReadingDto
import org.biodex.app.integrity.CanonicalPayload
import org.biodex.app.integrity.HardwareKeystoreSigner
import org.biodex.app.integrity.MoireAnalyzer
import org.biodex.app.integrity.SensorTelemetryTracker
import org.biodex.app.integrity.TelemetrySnapshot
import java.util.Base64
import java.util.UUID

enum class AlmanacCategoryFilter(
    val apiKey: String?,
    val label: String,
) {
    All(null, "Todas"),
    Birds("birds", "Aves"),
    Mammals("mammals", "Mamíferos"),
    ReptilesAmphibians("reptiles_amphibians", "Reptiles y Anfibios"),
    InsectsArachnids("insects_arachnids", "Insectos"),
    Aquatic("aquatic", "Acuáticos"),
}

enum class BioDexTab {
    Scanner,
    Collection,
    Battle,
    Friends,
    Profile,
}

data class BioDexUiState(
    val activeTab: BioDexTab = BioDexTab.Scanner,
    val playerId: String = "android-field-explorer",
    val telemetry: TelemetrySnapshot = TelemetrySnapshot(),
    val backendOnline: Boolean? = null,
    val backendStatusText: String = "Verificando servidor...",
    val activeBackendUrl: String = BuildConfig.API_BASE_URL,
    val customServerUrlInput: String = "",
    val isSubmitting: Boolean = false,
    val capturedJpegBytes: ByteArray? = null,
    val lastCaptureResponse: CaptureUploadResponse? = null,
    val errorMessage: String? = null,
    val almanacLoading: Boolean = false,
    val almanacTotalSpecies: Int = 0,
    val almanacDiscoveredCount: Int = 0,
    val almanacEntries: List<AlmanacItemDto> = emptyList(),
    val almanacSearchQuery: String = "",
    val almanacCategoryFilter: AlmanacCategoryFilter = AlmanacCategoryFilter.All,
    val almanacOnlyCaptured: Boolean = false,
    val selectedAlmanacEntry: AlmanacItemDto? = null,
    val almanacError: String? = null,
) {
    val filteredAlmanacEntries: List<AlmanacItemDto>
        get() {
            val query = almanacSearchQuery.trim().lowercase()
            val categoryKey = almanacCategoryFilter.apiKey
            return almanacEntries.filter { item ->
                val matchesCaptured = !almanacOnlyCaptured || item.discovered
                val matchesCategory = categoryKey == null || item.category == categoryKey
                val matchesQuery = query.isEmpty() ||
                    item.scientificName.lowercase().contains(query) ||
                    (item.commonName?.lowercase()?.contains(query) == true) ||
                    item.summary.lowercase().contains(query)
                matchesCaptured && matchesCategory && matchesQuery
            }
        }

    val totalCapturesCount: Long
        get() = almanacEntries.sumOf { it.captureCount }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BioDexUiState) return false
        return activeTab == other.activeTab &&
            playerId == other.playerId &&
            telemetry == other.telemetry &&
            backendOnline == other.backendOnline &&
            backendStatusText == other.backendStatusText &&
            activeBackendUrl == other.activeBackendUrl &&
            customServerUrlInput == other.customServerUrlInput &&
            isSubmitting == other.isSubmitting &&
            capturedJpegBytes.contentEquals(other.capturedJpegBytes) &&
            lastCaptureResponse == other.lastCaptureResponse &&
            errorMessage == other.errorMessage &&
            almanacLoading == other.almanacLoading &&
            almanacTotalSpecies == other.almanacTotalSpecies &&
            almanacDiscoveredCount == other.almanacDiscoveredCount &&
            almanacEntries == other.almanacEntries &&
            almanacSearchQuery == other.almanacSearchQuery &&
            almanacCategoryFilter == other.almanacCategoryFilter &&
            almanacOnlyCaptured == other.almanacOnlyCaptured &&
            selectedAlmanacEntry == other.selectedAlmanacEntry &&
            almanacError == other.almanacError
    }

    override fun hashCode(): Int {
        var result = activeTab.hashCode()
        result = 31 * result + playerId.hashCode()
        result = 31 * result + telemetry.hashCode()
        result = 31 * result + (backendOnline?.hashCode() ?: 0)
        result = 31 * result + backendStatusText.hashCode()
        result = 31 * result + activeBackendUrl.hashCode()
        result = 31 * result + customServerUrlInput.hashCode()
        result = 31 * result + isSubmitting.hashCode()
        result = 31 * result + (capturedJpegBytes?.contentHashCode() ?: 0)
        result = 31 * result + (lastCaptureResponse?.hashCode() ?: 0)
        result = 31 * result + (errorMessage?.hashCode() ?: 0)
        result = 31 * result + almanacLoading.hashCode()
        result = 31 * result + almanacTotalSpecies
        result = 31 * result + almanacDiscoveredCount
        result = 31 * result + almanacEntries.hashCode()
        result = 31 * result + almanacSearchQuery.hashCode()
        result = 31 * result + almanacCategoryFilter.hashCode()
        result = 31 * result + almanacOnlyCaptured.hashCode()
        result = 31 * result + (selectedAlmanacEntry?.hashCode() ?: 0)
        result = 31 * result + (almanacError?.hashCode() ?: 0)
        return result
    }
}

class BioDexViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = application.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    private val savedCustomUrl = prefs.getString(KEY_CUSTOM_SERVER_URL, "")?.trim().orEmpty()

    private val apiClient = BioDexApiClient(
        primaryUrl = BuildConfig.API_BASE_URL,
        fallbackLanUrl = BuildConfig.DEBUG_LAN_API_URL,
        customOverrideUrl = savedCustomUrl.ifEmpty { null },
    )
    private val keystoreSigner = HardwareKeystoreSigner()
    private val telemetryTracker = SensorTelemetryTracker(application)

    private val _uiState = MutableStateFlow(
        BioDexUiState(
            activeBackendUrl = apiClient.activeBaseUrl(),
            customServerUrlInput = savedCustomUrl,
        ),
    )
    val uiState: StateFlow<BioDexUiState> = _uiState.asStateFlow()

    init {
        telemetryTracker.startImu()
        viewModelScope.launch {
            telemetryTracker.snapshot.collect { snap ->
                _uiState.update { it.copy(telemetry = snap) }
            }
        }
        refreshConnectionAndAlmanac()
    }

    fun selectTab(tab: BioDexTab) {
        _uiState.update { it.copy(activeTab = tab, selectedAlmanacEntry = null) }
        if ((tab == BioDexTab.Collection || tab == BioDexTab.Battle || tab == BioDexTab.Profile) &&
            _uiState.value.almanacEntries.isEmpty()
        ) {
            refreshAlmanac()
        }
    }

    fun onLocationPermissionGranted() {
        telemetryTracker.startLocationUpdates()
    }

    fun refreshConnectionAndAlmanac() {
        viewModelScope.launch {
            val result = apiClient.checkHealth()
            _uiState.update { state ->
                result.fold(
                    onSuccess = { conn ->
                        state.copy(
                            backendOnline = true,
                            activeBackendUrl = conn.activeBaseUrl,
                            backendStatusText = "API + PostGIS OK (${shortHost(conn.activeBaseUrl)})",
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            backendOnline = false,
                            backendStatusText = "Sin conexión: ${err.message ?: "offline"}",
                        )
                    },
                )
            }
            if (result.isSuccess) {
                refreshAlmanac()
            }
        }
    }

    fun refreshBackendHealth() {
        refreshConnectionAndAlmanac()
    }

    fun updateCustomServerUrlInput(value: String) {
        _uiState.update { it.copy(customServerUrlInput = value) }
    }

    fun applyCustomServerUrl() {
        val raw = _uiState.value.customServerUrlInput.trim()
        prefs.edit().putString(KEY_CUSTOM_SERVER_URL, raw).apply()
        apiClient.setCustomBaseUrl(raw.ifEmpty { null })
        _uiState.update {
            it.copy(
                activeBackendUrl = apiClient.activeBaseUrl(),
                backendStatusText = "Probando servidor...",
            )
        }
        refreshConnectionAndAlmanac()
    }

    fun refreshAlmanac() {
        viewModelScope.launch {
            _uiState.update { it.copy(almanacLoading = true, almanacError = null) }
            val result = apiClient.fetchAlmanac(userId = _uiState.value.playerId)
            _uiState.update { state ->
                result.fold(
                    onSuccess = { almanac ->
                        val active = apiClient.activeBaseUrl()
                        state.copy(
                            almanacLoading = false,
                            almanacTotalSpecies = almanac.totalSpecies,
                            almanacDiscoveredCount = almanac.discoveredCount,
                            almanacEntries = almanac.entries,
                            activeBackendUrl = active,
                            backendOnline = true,
                            backendStatusText = "API + PostGIS OK (${shortHost(active)})",
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            almanacLoading = false,
                            almanacError = err.message ?: "No se pudo cargar la colección",
                        )
                    },
                )
            }
        }
    }

    fun updateAlmanacSearch(query: String) {
        _uiState.update { it.copy(almanacSearchQuery = query) }
    }

    fun selectAlmanacCategory(filter: AlmanacCategoryFilter) {
        _uiState.update { it.copy(almanacCategoryFilter = filter) }
    }

    fun toggleAlmanacOnlyCaptured() {
        _uiState.update { it.copy(almanacOnlyCaptured = !it.almanacOnlyCaptured) }
    }

    fun openAlmanacDetail(entry: AlmanacItemDto) {
        _uiState.update { it.copy(selectedAlmanacEntry = entry) }
    }

    fun closeAlmanacDetail() {
        _uiState.update { it.copy(selectedAlmanacEntry = null) }
    }

    fun clearResult() {
        _uiState.update {
            it.copy(
                lastCaptureResponse = null,
                capturedJpegBytes = null,
                errorMessage = null,
            )
        }
    }

    fun clearError() {
        _uiState.update { it.copy(errorMessage = null) }
    }

    fun submitCameraCapture(jpegBytes: ByteArray) {
        if (_uiState.value.isSubmitting) return

        viewModelScope.launch {
            _uiState.update { it.copy(isSubmitting = true, errorMessage = null) }

            val snapshot = _uiState.value.telemetry
            val lat = snapshot.latitude
            val lon = snapshot.longitude
            if (lat == null || lon == null) {
                _uiState.update {
                    it.copy(
                        isSubmitting = false,
                        errorMessage = "Esperando señal GPS real para validar ubicación.",
                    )
                }
                return@launch
            }

            val uploadResult = runCatching {
                withContext(Dispatchers.Default) {
                    val captureId = UUID.randomUUID().toString()
                    val timestampMs = System.currentTimeMillis()
                    val moireScore = MoireAnalyzer.estimateFromJpeg(jpegBytes)
                    val imageSha256 = CanonicalPayload.sha256(jpegBytes)

                    val canonicalBytes = CanonicalPayload.buildCaptureSigningBytes(
                        captureId = captureId,
                        capturedAtUtcMs = timestampMs,
                        sourceTag = CanonicalPayload.SOURCE_CAMERAX_LIVE,
                        latitude = lat,
                        longitude = lon,
                        accuracyMeters = snapshot.accuracyMeters,
                        accelXyz = snapshot.accelXyz,
                        gyroXyz = snapshot.gyroXyz,
                        moireScore = moireScore,
                        imageSha256 = imageSha256,
                    )
                    val signed = keystoreSigner.signPayload(canonicalBytes)

                    val b64 = Base64.getEncoder()
                    CaptureUploadRequest(
                        userId = _uiState.value.playerId,
                        imageBase64 = b64.encodeToString(jpegBytes),
                        metadata = CaptureMetadataDto(
                            captureId = captureId,
                            capturedAtUtcMs = timestampMs,
                            source = "camera_x_live",
                            coordinates = GeoPointDto(
                                latitude = lat,
                                longitude = lon,
                                accuracyMeters = snapshot.accuracyMeters,
                            ),
                            imu = ImuReadingDto(
                                accelXyz = snapshot.accelXyz.toList(),
                                gyroXyz = snapshot.gyroXyz.toList(),
                            ),
                            moireScore = moireScore,
                            imageSha256Base64 = b64.encodeToString(imageSha256),
                            devicePublicKeyBase64 = b64.encodeToString(signed.publicKeySpkiDer),
                            signatureBase64 = b64.encodeToString(signed.signatureDer),
                        ),
                    )
                }
            }.fold(
                onSuccess = { request -> apiClient.submitCapture(request) },
                onFailure = { err -> Result.failure(err) },
            )

            _uiState.update { state ->
                val active = apiClient.activeBaseUrl()
                uploadResult.fold(
                    onSuccess = { response ->
                        state.copy(
                            isSubmitting = false,
                            capturedJpegBytes = jpegBytes,
                            lastCaptureResponse = response,
                            activeBackendUrl = active,
                            backendOnline = true,
                            backendStatusText = "API + PostGIS OK (${shortHost(active)})",
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            isSubmitting = false,
                            errorMessage = err.message ?: "Error al procesar captura",
                        )
                    },
                )
            }

            if (uploadResult.isSuccess) {
                refreshAlmanac()
            }
        }
    }

    override fun onCleared() {
        telemetryTracker.stop()
        super.onCleared()
    }

    private fun shortHost(url: String): String =
        url.removePrefix("http://").removePrefix("https://")

    private companion object {
        const val PREFS_NAME = "biodex_prefs"
        const val KEY_CUSTOM_SERVER_URL = "custom_server_url"
    }
}
