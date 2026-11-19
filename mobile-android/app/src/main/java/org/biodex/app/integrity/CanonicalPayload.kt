package org.biodex.app.integrity

import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.charset.StandardCharsets
import java.security.MessageDigest

/**
 * Builds the deterministic `BIODEX_CAPTURE_V1` byte buffer matching
 * `biodex_core::crypto::CaptureMetadata::canonical_signing_bytes()` bit-for-bit.
 */
object CanonicalPayload {
    private val DOMAIN_TAG = "BIODEX_CAPTURE_V1".toByteArray(StandardCharsets.US_ASCII)
    const val SOURCE_CAMERAX_LIVE: Byte = 0x01
    const val SOURCE_GALLERY: Byte = 0x02

    fun sha256(bytes: ByteArray): ByteArray =
        MessageDigest.getInstance("SHA-256").digest(bytes)

    fun buildCaptureSigningBytes(
        captureId: String,
        capturedAtUtcMs: Long,
        sourceTag: Byte,
        latitude: Double,
        longitude: Double,
        accuracyMeters: Float,
        accelXyz: FloatArray,
        gyroXyz: FloatArray,
        moireScore: Float,
        imageSha256: ByteArray,
    ): ByteArray {
        require(accelXyz.size == 3) { "accelXyz must have length 3" }
        require(gyroXyz.size == 3) { "gyroXyz must have length 3" }
        require(imageSha256.size == 32) { "imageSha256 must be 32 bytes" }

        val idBytes = captureId.toByteArray(StandardCharsets.UTF_8)
        require(idBytes.isNotEmpty() && idBytes.size <= 256) { "Invalid captureId length" }

        val totalCapacity = DOMAIN_TAG.size + 4 + idBytes.size + 8 + 1 + 8 + 8 + 4 + 12 + 12 + 4 + 32
        val buffer = ByteBuffer.allocate(totalCapacity).order(ByteOrder.BIG_ENDIAN)

        buffer.put(DOMAIN_TAG)
        buffer.putInt(idBytes.size)
        buffer.put(idBytes)
        buffer.putLong(capturedAtUtcMs)
        buffer.put(sourceTag)
        buffer.putLong(canonicalDoubleBits(latitude))
        buffer.putLong(canonicalDoubleBits(longitude))
        buffer.putInt(canonicalFloatBits(accuracyMeters))
        accelXyz.forEach { buffer.putInt(canonicalFloatBits(it)) }
        gyroXyz.forEach { buffer.putInt(canonicalFloatBits(it)) }
        buffer.putInt(canonicalFloatBits(moireScore))
        buffer.put(imageSha256)

        return buffer.array()
    }

    private fun canonicalDoubleBits(value: Double): Long {
        require(value.isFinite()) { "Non-finite double in canonical payload" }
        val normalized = if (value == 0.0) 0.0 else value
        return java.lang.Double.doubleToRawLongBits(normalized)
    }

    private fun canonicalFloatBits(value: Float): Int {
        require(value.isFinite()) { "Non-finite float in canonical payload" }
        val normalized = if (value == 0.0f) 0.0f else value
        return java.lang.Float.floatToRawIntBits(normalized)
    }
}
