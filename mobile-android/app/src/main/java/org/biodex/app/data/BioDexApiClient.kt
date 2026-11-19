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

class BioDexApiClient(
    private val baseUrl: String,
) {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = false
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(25, TimeUnit.SECONDS)
        .writeTimeout(25, TimeUnit.SECONDS)
        .build()

    suspend fun checkHealth(): Result<HealthResponse> = withContext(Dispatchers.IO) {
        runCatching {
            val req = Request.Builder()
                .url("$baseUrl/health")
                .get()
                .build()
            client.newCall(req).execute().use { resp ->
                val body = resp.body?.string().orEmpty()
                if (!resp.isSuccessful) {
                    throw IOException("Backend HTTP ${resp.code}: $body")
                }
                json.decodeFromString<HealthResponse>(body)
            }
        }
    }

    suspend fun fetchAlmanac(userId: String? = null): Result<AlmanacListResponse> =
        withContext(Dispatchers.IO) {
            runCatching {
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
                        throw IOException("Almanaque HTTP ${resp.code}: $body")
                    }
                    json.decodeFromString<AlmanacListResponse>(body)
                }
            }
        }

    suspend fun submitCapture(payload: CaptureUploadRequest): Result<CaptureUploadResponse> =
        withContext(Dispatchers.IO) {
            runCatching {
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
                        throw IOException(msg)
                    }
                    json.decodeFromString<CaptureUploadResponse>(body)
                }
            }
        }
}
