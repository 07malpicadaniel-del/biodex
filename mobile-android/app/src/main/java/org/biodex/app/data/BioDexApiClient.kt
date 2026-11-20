package org.biodex.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

data class HealthConnectionResult(
    val activeBaseUrl: String,
    val health: HealthResponse,
)

class BioDexApiClient(
    primaryUrl: String,
    fallbackLanUrl: String = "",
    customOverrideUrl: String? = null,
) {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = false
    }

    private val probeClient = OkHttpClient.Builder()
        .connectTimeout(3, TimeUnit.SECONDS)
        .readTimeout(4, TimeUnit.SECONDS)
        .build()

    private val client = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(45, TimeUnit.SECONDS)
        .writeTimeout(25, TimeUnit.SECONDS)
        .build()

    private val primaryRef = AtomicReference(normalizeBaseUrl(primaryUrl))
    private val fallbackRef = AtomicReference(normalizeBaseUrl(fallbackLanUrl))
    private val customRef = AtomicReference(customOverrideUrl?.let(::normalizeBaseUrl))
    private val activeBaseUrlRef = AtomicReference(
        buildCandidateUrls(
            customUrl = customRef.get(),
            primaryUrl = primaryRef.get(),
            fallbackUrl = fallbackRef.get(),
        ).firstOrNull() ?: "http://127.0.0.1:3030",
    )

    fun activeBaseUrl(): String = activeBaseUrlRef.get()

    fun setCustomBaseUrl(customUrl: String?) {
        val normalized = customUrl?.trim()?.takeIf { it.isNotEmpty() }?.let(::normalizeBaseUrl)
        customRef.set(normalized)
        if (normalized != null) {
            activeBaseUrlRef.set(normalized)
        }
    }

    suspend fun checkHealth(): Result<HealthConnectionResult> = withContext(Dispatchers.IO) {
        runCatching {
            val candidates = buildCandidateUrls(
                customUrl = customRef.get(),
                primaryUrl = primaryRef.get(),
                fallbackUrl = fallbackRef.get(),
                preferredActive = activeBaseUrlRef.get(),
            )
            var lastFailure: Throwable? = null
            for (baseUrl in candidates) {
                val attempt = runCatching { fetchHealthFrom(baseUrl) }
                if (attempt.isSuccess) {
                    activeBaseUrlRef.set(baseUrl)
                    return@runCatching HealthConnectionResult(
                        activeBaseUrl = baseUrl,
                        health = attempt.getOrThrow(),
                    )
                }
                lastFailure = attempt.exceptionOrNull()
            }
            throw (lastFailure ?: IOException("No se encontró ningún servidor BioDex accesible en el puerto 3030"))
        }
    }

    suspend fun fetchAlmanac(
        userId: String? = null,
        devMode: Boolean = false,
        query: String? = null,
        category: String? = null,
        capturedFilter: String? = null,
        limit: Int? = 60,
        offset: Int = 0,
    ): Result<AlmanacListResponse> =
        executeGet("/api/v1/almanac") { builder ->
            if (!userId.isNullOrBlank()) {
                builder.addQueryParameter("user_id", userId)
            }
            if (devMode) {
                builder.addQueryParameter("dev_mode", "true")
            }
            if (!query.isNullOrBlank()) {
                builder.addQueryParameter("q", query.trim())
            }
            if (!category.isNullOrBlank()) {
                builder.addQueryParameter("category", category)
            }
            if (!capturedFilter.isNullOrBlank()) {
                builder.addQueryParameter("captured_filter", capturedFilter)
            }
            if (limit != null) {
                builder.addQueryParameter("limit", limit.toString())
            }
            if (offset > 0) {
                builder.addQueryParameter("offset", offset.toString())
            }
        }

    suspend fun syncNaturaListaSpecies(
        page: Int = 1,
        category: String? = null,
    ): Result<NaturaListaSyncResponse> =
        withContext(Dispatchers.IO) {
            executeWithEndpointFailover { baseUrl ->
                val builder = "$baseUrl/api/v1/almanac/naturalista/sync".toHttpUrl().newBuilder()
                builder.addQueryParameter("page", page.toString())
                if (!category.isNullOrBlank()) {
                    builder.addQueryParameter("category", category)
                }
                val emptyBody = ByteArray(0).toRequestBody(null)
                val req = Request.Builder().url(builder.build()).post(emptyBody).build()
                client.newCall(req).execute().use { resp ->
                    val body = resp.body?.string().orEmpty()
                    if (!resp.isSuccessful) {
                        throw ApiHttpException(friendlyApiError(resp.code, body))
                    }
                    json.decodeFromString<NaturaListaSyncResponse>(body)
                }
            }
        }

    suspend fun searchNaturaLista(
        query: String,
        userId: String? = null,
        devMode: Boolean = false,
    ): Result<NaturaListaDiscoveryResponse> =
        executeGet("/api/v1/almanac/naturalista/search") { builder ->
            builder.addQueryParameter("q", query.trim())
            if (!userId.isNullOrBlank()) {
                builder.addQueryParameter("user_id", userId)
            }
            if (devMode) {
                builder.addQueryParameter("dev_mode", "true")
            }
        }

    suspend fun fetchNearbyNaturaLista(
        latitude: Double,
        longitude: Double,
        radiusKm: Int = 25,
        userId: String? = null,
        devMode: Boolean = false,
    ): Result<NaturaListaDiscoveryResponse> =
        executeGet("/api/v1/almanac/naturalista/nearby") { builder ->
            builder.addQueryParameter("lat", latitude.toString())
            builder.addQueryParameter("lng", longitude.toString())
            builder.addQueryParameter("radius_km", radiusKm.toString())
            if (!userId.isNullOrBlank()) {
                builder.addQueryParameter("user_id", userId)
            }
            if (devMode) {
                builder.addQueryParameter("dev_mode", "true")
            }
        }

    suspend fun inspectNaturaListaTaxon(
        inaturalistId: Long,
        userId: String? = null,
        devMode: Boolean = false,
    ): Result<AlmanacItemDto> =
        executeGet("/api/v1/almanac/naturalista/taxon/$inaturalistId") { builder ->
            if (!userId.isNullOrBlank()) {
                builder.addQueryParameter("user_id", userId)
            }
            if (devMode) {
                builder.addQueryParameter("dev_mode", "true")
            }
        }

    suspend fun submitCapture(payload: CaptureUploadRequest): Result<CaptureUploadResponse> =
        withContext(Dispatchers.IO) {
            executeWithEndpointFailover { baseUrl ->
                val serialized = json.encodeToString(payload)
                val req = Request.Builder()
                    .url("$baseUrl/api/v1/captures")
                    .post(serialized.toRequestBody("application/json; charset=utf-8".toMediaType()))
                    .build()

                client.newCall(req).execute().use { resp ->
                    val body = resp.body?.string().orEmpty()
                    if (!resp.isSuccessful) {
                        throw ApiHttpException(friendlyApiError(resp.code, body))
                    }
                    json.decodeFromString<CaptureUploadResponse>(body)
                }
            }
        }

    private suspend inline fun <reified T> executeGet(
        path: String,
        crossinline configureQuery: (HttpUrl.Builder) -> Unit,
    ): Result<T> = withContext(Dispatchers.IO) {
        executeWithEndpointFailover { baseUrl ->
            val builder = "$baseUrl$path".toHttpUrl().newBuilder()
            configureQuery(builder)
            val req = Request.Builder().url(builder.build()).get().build()
            client.newCall(req).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful) {
                    throw ApiHttpException(friendlyApiError(resp.code, body))
                }
                json.decodeFromString<T>(body)
            }
        }
    }

    private fun friendlyApiError(httpCode: Int, body: String): String {
        val parsed = runCatching { json.decodeFromString<ApiErrorEnvelope>(body).error }.getOrNull()
        return when (parsed?.code) {
            "vision_not_configured" ->
                "Pendiente configurar credenciales de visión en el backend."
            "taxonomic_rank_too_broad" ->
                "Acércate un poco más o enfoca mejor: solo se pudo distinguir un grupo general."
            "no_species_detected" ->
                "No se identificó ninguna especie con suficiente claridad. Intenta otra toma."
            "non_fauna_detected" ->
                "BioDex solo registra fauna animal; enfoca directamente un animal."
            else -> parsed?.let { "${it.code}: ${it.message}" } ?: "HTTP $httpCode: $body"
        }
    }

    private fun <T> executeWithEndpointFailover(block: (String) -> T): Result<T> = runCatching {
        val current = activeBaseUrlRef.get()
        try {
            block(current)
        } catch (httpErr: ApiHttpException) {
            throw httpErr
        } catch (networkErr: IOException) {
            val candidates = buildCandidateUrls(
                customUrl = customRef.get(),
                primaryUrl = primaryRef.get(),
                fallbackUrl = fallbackRef.get(),
            ).filter { it != current }

            for (fallback in candidates) {
                val healthy = runCatching { fetchHealthFrom(fallback) }.isSuccess
                if (healthy) {
                    activeBaseUrlRef.set(fallback)
                    return@runCatching block(fallback)
                }
            }
            throw networkErr
        }
    }

    private fun fetchHealthFrom(baseUrl: String): HealthResponse {
        val req = Request.Builder()
            .url("$baseUrl/health")
            .get()
            .build()
        probeClient.newCall(req).execute().use { resp ->
            val body = resp.body?.string().orEmpty()
            if (!resp.isSuccessful) {
                throw ApiHttpException("Backend HTTP ${resp.code}: $body")
            }
            return json.decodeFromString<HealthResponse>(body)
        }
    }

    private class ApiHttpException(message: String) : IOException(message)

    companion object {
        fun normalizeBaseUrl(raw: String): String {
            val trimmed = raw.trim().trimEnd('/')
            if (trimmed.isEmpty()) return ""
            return if (trimmed.startsWith("http://") || trimmed.startsWith("https://")) {
                trimmed
            } else {
                "http://$trimmed"
            }
        }

        fun buildCandidateUrls(
            customUrl: String?,
            primaryUrl: String,
            fallbackUrl: String,
            preferredActive: String? = null,
        ): List<String> {
            return listOfNotNull(
                customUrl?.takeIf { it.isNotBlank() },
                preferredActive?.takeIf { it.isNotBlank() },
                primaryUrl.takeIf { it.isNotBlank() },
                fallbackUrl.takeIf { it.isNotBlank() },
            ).map(::normalizeBaseUrl).distinct()
        }
    }
}
