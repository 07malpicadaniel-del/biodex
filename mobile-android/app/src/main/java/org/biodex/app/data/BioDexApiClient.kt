package org.biodex.app.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
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
        .readTimeout(25, TimeUnit.SECONDS)
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
        ).firstOrNull() ?: "http://127.0.0.1:3000",
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
            throw (lastFailure ?: IOException("No se encontró ningún servidor BioDex accesible en el puerto 3000"))
        }
    }

    suspend fun fetchAlmanac(userId: String? = null): Result<AlmanacListResponse> =
        withContext(Dispatchers.IO) {
            executeWithEndpointFailover { baseUrl ->
                val urlBuilder = "$baseUrl/api/v1/almanac".toHttpUrl().newBuilder()
                if (!userId.isNullOrBlank()) {
                    urlBuilder.addQueryParameter("user_id", userId)
                }
                val req = Request.Builder()
                    .url(urlBuilder.build())
                    .get()
                    .build()

                client.newCall(req).execute().use { resp ->
                    val body = resp.body?.string().orEmpty()
                    if (!resp.isSuccessful) {
                        throw ApiHttpException("Almanaque HTTP ${resp.code}: $body")
                    }
                    json.decodeFromString<AlmanacListResponse>(body)
                }
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
                        val parsedError = runCatching {
                            json.decodeFromString<ApiErrorEnvelope>(body).error
                        }.getOrNull()
                        val msg = when (parsedError?.code) {
                            "vision_not_configured" ->
                                "Pendiente configurar credenciales de visión en el backend."
                            "taxonomic_rank_too_broad" ->
                                "Acércate un poco más o enfoca mejor: solo se pudo distinguir un grupo general."
                            "no_species_detected" ->
                                "No se identificó ninguna especie con suficiente claridad. Intenta otra toma."
                            else -> parsedError?.let { "${it.code}: ${it.message}" }
                                ?: "HTTP ${resp.code}: $body"
                        }
                        throw ApiHttpException(msg)
                    }
                    json.decodeFromString<CaptureUploadResponse>(body)
                }
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
