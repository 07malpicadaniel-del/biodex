package org.biodex.app.ui

import android.Manifest
import android.app.Application
import android.content.Context
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
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
import org.biodex.app.data.NaturaListaDiscoveryResponse
import org.biodex.app.data.NaturaListaSpeciesDto
import org.biodex.app.integrity.CanonicalPayload
import org.biodex.app.integrity.CaptureImageProcessor
import org.biodex.app.integrity.HardwareKeystoreSigner
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

enum class AlmanacCaptureFilter(val label: String) {
    All("Todas"),
    Captured("Capturadas"),
    WildOnly("Salvajes"),
    SanctuaryOnly("Santuario"),
    Undiscovered("Por descubrir"),
}

enum class CollectionSubTab(val label: String) {
    Almanac("Mi Colección"),
    Nearby("Cerca"),
    Search("NaturaLista"),
}

enum class BioDexTab {
    Scanner,
    Collection,
    Battle,
    Friends,
    Profile,
}

data class BioDexUiState(
    val isLoggedIn: Boolean = false,
    val explorerNameInput: String = DEFAULT_EXPLORER_ID,
    val loginDevModeSelected: Boolean = false,
    val isDevMode: Boolean = false,
    val activeTab: BioDexTab = BioDexTab.Scanner,
    val playerId: String = DEFAULT_EXPLORER_ID,
    val telemetry: TelemetrySnapshot = TelemetrySnapshot(),
    val backendOnline: Boolean? = null,
    val backendStatusText: String = "Verificando servidor...",
    val activeBackendUrl: String = BuildConfig.API_BASE_URL,
    val customServerUrlInput: String = "",
    val isSubmitting: Boolean = false,
    val capturedJpegBytes: ByteArray? = null,
    val lastCaptureResponse: CaptureUploadResponse? = null,
    val errorMessage: String? = null,
    val collectionSubTab: CollectionSubTab = CollectionSubTab.Almanac,
    val almanacLoading: Boolean = false,
    val almanacTotalSpecies: Int = 0,
    val almanacDiscoveredCount: Int = 0,
    val almanacEntries: List<AlmanacItemDto> = emptyList(),
    val almanacSearchQuery: String = "",
    val almanacCategoryFilter: AlmanacCategoryFilter = AlmanacCategoryFilter.All,
    val almanacCaptureFilter: AlmanacCaptureFilter = AlmanacCaptureFilter.All,
    val selectedAlmanacEntry: AlmanacItemDto? = null,
    val almanacError: String? = null,
    val naturalistaSearchQuery: String = "",
    val naturalistaSearchLoading: Boolean = false,
    val naturalistaSearchResults: NaturaListaDiscoveryResponse? = null,
    val naturalistaSearchError: String? = null,
    val naturalistaNearbyLoading: Boolean = false,
    val naturalistaNearbyResults: NaturaListaDiscoveryResponse? = null,
    val naturalistaNearbyError: String? = null,
    val inspectingTaxonLoading: Boolean = false,
) {
    val filteredAlmanacEntries: List<AlmanacItemDto>
        get() {
            val query = almanacSearchQuery.trim().lowercase()
            val categoryKey = almanacCategoryFilter.apiKey
            return almanacEntries.filter { item ->
                val matchesCapture = when (almanacCaptureFilter) {
                    AlmanacCaptureFilter.All -> true
                    AlmanacCaptureFilter.Captured -> item.discovered
                    AlmanacCaptureFilter.WildOnly -> item.discovered && item.bestVariant == "wild"
                    AlmanacCaptureFilter.SanctuaryOnly -> item.discovered && item.bestVariant == "sanctuary"
                    AlmanacCaptureFilter.Undiscovered -> !item.discovered
                }
                val matchesCategory = categoryKey == null || item.category == categoryKey
                val matchesQuery = query.isEmpty() ||
                    item.scientificName.lowercase().contains(query) ||
                    (item.commonName?.lowercase()?.contains(query) == true) ||
                    item.summary.lowercase().contains(query)
                matchesCapture && matchesCategory && matchesQuery
            }
        }

    val totalCapturesCount: Long
        get() = almanacEntries.sumOf { it.captureCount }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is BioDexUiState) return false
        return isLoggedIn == other.isLoggedIn &&
            explorerNameInput == other.explorerNameInput &&
            loginDevModeSelected == other.loginDevModeSelected &&
            isDevMode == other.isDevMode &&
            activeTab == other.activeTab &&
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
            collectionSubTab == other.collectionSubTab &&
            almanacLoading == other.almanacLoading &&
            almanacTotalSpecies == other.almanacTotalSpecies &&
            almanacDiscoveredCount == other.almanacDiscoveredCount &&
            almanacEntries == other.almanacEntries &&
            almanacSearchQuery == other.almanacSearchQuery &&
            almanacCategoryFilter == other.almanacCategoryFilter &&
            almanacCaptureFilter == other.almanacCaptureFilter &&
            selectedAlmanacEntry == other.selectedAlmanacEntry &&
            almanacError == other.almanacError &&
            naturalistaSearchQuery == other.naturalistaSearchQuery &&
            naturalistaSearchLoading == other.naturalistaSearchLoading &&
            naturalistaSearchResults == other.naturalistaSearchResults &&
            naturalistaSearchError == other.naturalistaSearchError &&
            naturalistaNearbyLoading == other.naturalistaNearbyLoading &&
            naturalistaNearbyResults == other.naturalistaNearbyResults &&
            naturalistaNearbyError == other.naturalistaNearbyError &&
            inspectingTaxonLoading == other.inspectingTaxonLoading
    }

    override fun hashCode(): Int {
        var result = isLoggedIn.hashCode()
        result = 31 * result + explorerNameInput.hashCode()
        result = 31 * result + loginDevModeSelected.hashCode()
        result = 31 * result + isDevMode.hashCode()
        result = 31 * result + activeTab.hashCode()
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
        result = 31 * result + collectionSubTab.hashCode()
        result = 31 * result + almanacLoading.hashCode()
        result = 31 * result + almanacTotalSpecies
        result = 31 * result + almanacDiscoveredCount
        result = 31 * result + almanacEntries.hashCode()
        result = 31 * result + almanacSearchQuery.hashCode()
        result = 31 * result + almanacCategoryFilter.hashCode()
        result = 31 * result + almanacCaptureFilter.hashCode()
        result = 31 * result + (selectedAlmanacEntry?.hashCode() ?: 0)
        result = 31 * result + (almanacError?.hashCode() ?: 0)
        result = 31 * result + naturalistaSearchQuery.hashCode()
        result = 31 * result + naturalistaSearchLoading.hashCode()
        result = 31 * result + (naturalistaSearchResults?.hashCode() ?: 0)
        result = 31 * result + (naturalistaSearchError?.hashCode() ?: 0)
        result = 31 * result + naturalistaNearbyLoading.hashCode()
        result = 31 * result + (naturalistaNearbyResults?.hashCode() ?: 0)
        result = 31 * result + (naturalistaNearbyError?.hashCode() ?: 0)
        result = 31 * result + inspectingTaxonLoading.hashCode()
        return result
    }

    companion object {
        const val DEFAULT_EXPLORER_ID = "android-field-explorer"
    }
}

class BioDexViewModel(application: Application) : AndroidViewModel(application) {
    private val prefs = application.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    private val savedCustomUrl = prefs.getString(KEY_CUSTOM_SERVER_URL, "")?.trim().orEmpty()
    private val savedExplorerId = prefs.getString(KEY_LAST_EXPLORER_ID, "")
        ?.trim()
        ?.takeIf { it.isNotEmpty() }
        ?: BioDexUiState.DEFAULT_EXPLORER_ID

    private val apiClient = BioDexApiClient(
        primaryUrl = BuildConfig.API_BASE_URL,
        fallbackLanUrl = BuildConfig.DEBUG_LAN_API_URL,
        customOverrideUrl = savedCustomUrl.ifEmpty { null },
    )
    private val keystoreSigner = HardwareKeystoreSigner()
    private val telemetryTracker = SensorTelemetryTracker(application)

    private val _uiState = MutableStateFlow(
        BioDexUiState(
            isLoggedIn = false,
            explorerNameInput = savedExplorerId,
            playerId = savedExplorerId,
            activeBackendUrl = apiClient.activeBaseUrl(),
            customServerUrlInput = savedCustomUrl,
        ),
    )
    val uiState: StateFlow<BioDexUiState> = _uiState.asStateFlow()

    init {
        telemetryTracker.startImu()
        startLocationUpdatesIfPermitted()
        viewModelScope.launch {
            telemetryTracker.snapshot.collect { snap ->
                _uiState.update { it.copy(telemetry = snap) }
            }
        }
        refreshConnectionHealthOnly()
    }

    private fun startLocationUpdatesIfPermitted() {
        val ctx = getApplication<Application>()
        val fineGranted = ContextCompat.checkSelfPermission(
            ctx,
            Manifest.permission.ACCESS_FINE_LOCATION,
        ) == PackageManager.PERMISSION_GRANTED
        val coarseGranted = ContextCompat.checkSelfPermission(
            ctx,
            Manifest.permission.ACCESS_COARSE_LOCATION,
        ) == PackageManager.PERMISSION_GRANTED
        if (fineGranted || coarseGranted) {
            telemetryTracker.startLocationUpdates()
        }
    }

    fun updateExplorerNameInput(value: String) {
        _uiState.update { it.copy(explorerNameInput = value) }
    }

    fun setLoginDevModeSelected(enabled: Boolean) {
        _uiState.update { it.copy(loginDevModeSelected = enabled) }
    }

    fun login() {
        val stateSnapshot = _uiState.value
        val resolvedPlayer = stateSnapshot.explorerNameInput.trim().ifEmpty {
            BioDexUiState.DEFAULT_EXPLORER_ID
        }
        prefs.edit().putString(KEY_LAST_EXPLORER_ID, resolvedPlayer).apply()

        _uiState.update {
            it.copy(
                isLoggedIn = true,
                playerId = resolvedPlayer,
                explorerNameInput = resolvedPlayer,
                isDevMode = it.loginDevModeSelected,
                activeTab = if (it.loginDevModeSelected) BioDexTab.Collection else BioDexTab.Scanner,
                collectionSubTab = CollectionSubTab.Almanac,
                almanacCaptureFilter = AlmanacCaptureFilter.All,
                almanacEntries = emptyList(),
                naturalistaSearchResults = null,
                naturalistaNearbyResults = null,
                selectedAlmanacEntry = null,
                lastCaptureResponse = null,
                capturedJpegBytes = null,
                errorMessage = null,
            )
        }
        refreshConnectionAndAlmanac()
        startLocationUpdatesIfPermitted()
    }

    fun logout() {
        _uiState.update {
            it.copy(
                isLoggedIn = false,
                selectedAlmanacEntry = null,
                lastCaptureResponse = null,
                capturedJpegBytes = null,
                errorMessage = null,
                almanacEntries = emptyList(),
                naturalistaSearchResults = null,
                naturalistaNearbyResults = null,
            )
        }
        refreshConnectionHealthOnly()
    }

    fun selectTab(tab: BioDexTab) {
        _uiState.update { it.copy(activeTab = tab, selectedAlmanacEntry = null) }
        if ((tab == BioDexTab.Collection || tab == BioDexTab.Profile) &&
            _uiState.value.almanacEntries.isEmpty()
        ) {
            refreshAlmanac()
        }
    }

    fun selectCollectionSubTab(subTab: CollectionSubTab) {
        _uiState.update { it.copy(collectionSubTab = subTab) }
        if (subTab == CollectionSubTab.Nearby &&
            _uiState.value.naturalistaNearbyResults == null &&
            !_uiState.value.naturalistaNearbyLoading
        ) {
            refreshNearbyNaturaLista()
        }
    }

    fun goToCollectionFromResult() {
        _uiState.update {
            it.copy(
                activeTab = BioDexTab.Collection,
                collectionSubTab = CollectionSubTab.Almanac,
                lastCaptureResponse = null,
                capturedJpegBytes = null,
                selectedAlmanacEntry = null,
                errorMessage = null,
            )
        }
        refreshAlmanac()
    }

    fun onLocationPermissionGranted() {
        telemetryTracker.startLocationUpdates()
    }

    fun refreshConnectionHealthOnly() {
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
        }
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
            if (result.isSuccess && _uiState.value.isLoggedIn) {
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
        val stateSnapshot = _uiState.value
        viewModelScope.launch {
            _uiState.update { it.copy(almanacLoading = true, almanacError = null) }
            val result = apiClient.fetchAlmanac(
                userId = stateSnapshot.playerId,
                devMode = stateSnapshot.isDevMode,
            )
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

    fun updateNaturaListaSearchQuery(query: String) {
        _uiState.update { it.copy(naturalistaSearchQuery = query) }
    }

    fun submitNaturaListaSearch() {
        val stateSnapshot = _uiState.value
        val query = stateSnapshot.naturalistaSearchQuery.trim()
        if (query.length < 2) {
            _uiState.update {
                it.copy(naturalistaSearchError = "Escribe al menos 2 letras para buscar en NaturaLista.")
            }
            return
        }
        viewModelScope.launch {
            _uiState.update {
                it.copy(naturalistaSearchLoading = true, naturalistaSearchError = null)
            }
            val result = apiClient.searchNaturaLista(
                query = query,
                userId = stateSnapshot.playerId,
                devMode = stateSnapshot.isDevMode,
            )
            _uiState.update { state ->
                result.fold(
                    onSuccess = { resp ->
                        state.copy(
                            naturalistaSearchLoading = false,
                            naturalistaSearchResults = resp,
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            naturalistaSearchLoading = false,
                            naturalistaSearchError = err.message ?: "Error al consultar NaturaLista",
                        )
                    },
                )
            }
        }
    }

    fun refreshNearbyNaturaLista(radiusKm: Int = 25) {
        val stateSnapshot = _uiState.value
        val lat = stateSnapshot.telemetry.latitude
        val lon = stateSnapshot.telemetry.longitude
        if (lat == null || lon == null) {
            _uiState.update {
                it.copy(
                    naturalistaNearbyError =
                        "Activa los permisos de ubicación y espera la señal GPS para consultar fauna cercana en NaturaLista.",
                )
            }
            return
        }
        viewModelScope.launch {
            _uiState.update {
                it.copy(naturalistaNearbyLoading = true, naturalistaNearbyError = null)
            }
            val result = apiClient.fetchNearbyNaturaLista(
                latitude = lat,
                longitude = lon,
                radiusKm = radiusKm,
                userId = stateSnapshot.playerId,
                devMode = stateSnapshot.isDevMode,
            )
            _uiState.update { state ->
                result.fold(
                    onSuccess = { resp ->
                        state.copy(
                            naturalistaNearbyLoading = false,
                            naturalistaNearbyResults = resp,
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            naturalistaNearbyLoading = false,
                            naturalistaNearbyError = err.message
                                ?: "No se pudieron obtener avistamientos cercanos",
                        )
                    },
                )
            }
        }
    }

    fun inspectNaturaListaSpecies(species: NaturaListaSpeciesDto) {
        val stateSnapshot = _uiState.value
        if (stateSnapshot.inspectingTaxonLoading) return
        viewModelScope.launch {
            _uiState.update { it.copy(inspectingTaxonLoading = true) }
            val result = apiClient.inspectNaturaListaTaxon(
                inaturalistId = species.taxonId.id,
                userId = stateSnapshot.playerId,
                devMode = stateSnapshot.isDevMode,
            )
            _uiState.update { state ->
                result.fold(
                    onSuccess = { entry ->
                        state.copy(
                            inspectingTaxonLoading = false,
                            selectedAlmanacEntry = entry,
                        )
                    },
                    onFailure = { err ->
                        state.copy(
                            inspectingTaxonLoading = false,
                            almanacError = err.message ?: "No se pudo cargar la ficha de NaturaLista",
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

    fun selectAlmanacCaptureFilter(filter: AlmanacCaptureFilter) {
        _uiState.update { it.copy(almanacCaptureFilter = filter) }
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

    fun reportCaptureError(message: String) {
        _uiState.update { it.copy(isSubmitting = false, errorMessage = message) }
    }

    fun submitCameraCapture(rawJpegBytes: ByteArray) {
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

            var uploadedBytes: ByteArray = rawJpegBytes
            val uploadResult = runCatching {
                withContext(Dispatchers.Default) {
                    val prepared = CaptureImageProcessor.prepareLiveCapture(rawJpegBytes)
                    uploadedBytes = prepared.uploadJpegBytes

                    val captureId = UUID.randomUUID().toString()
                    val timestampMs = System.currentTimeMillis()
                    val imageSha256 = CanonicalPayload.sha256(prepared.uploadJpegBytes)

                    val canonicalBytes = CanonicalPayload.buildCaptureSigningBytes(
                        captureId = captureId,
                        capturedAtUtcMs = timestampMs,
                        sourceTag = CanonicalPayload.SOURCE_CAMERAX_LIVE,
                        latitude = lat,
                        longitude = lon,
                        accuracyMeters = snapshot.accuracyMeters,
                        accelXyz = snapshot.accelXyz,
                        gyroXyz = snapshot.gyroXyz,
                        moireScore = prepared.moireScore,
                        imageSha256 = imageSha256,
                    )
                    val signed = keystoreSigner.signPayload(canonicalBytes)

                    val b64 = Base64.getEncoder()
                    CaptureUploadRequest(
                        userId = _uiState.value.playerId,
                        imageBase64 = b64.encodeToString(prepared.uploadJpegBytes),
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
                            moireScore = prepared.moireScore,
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
                            capturedJpegBytes = uploadedBytes,
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
        const val KEY_LAST_EXPLORER_ID = "last_explorer_id"
    }
}
