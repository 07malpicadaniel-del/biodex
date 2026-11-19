package org.biodex.app

import org.biodex.app.integrity.CanonicalPayload
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.security.KeyPairGenerator
import java.security.Signature
import java.security.spec.ECGenParameterSpec

class CanonicalPayloadTest {
    @Test
    fun normalizesNegativeZeroToPositiveZeroBits() {
        val digest = ByteArray(32) { 0x11 }
        val payloadNeg = CanonicalPayload.buildCaptureSigningBytes(
            captureId = "cap-1",
            capturedAtUtcMs = 1_795_017_600_000L,
            sourceTag = CanonicalPayload.SOURCE_CAMERAX_LIVE,
            latitude = -0.0,
            longitude = 19.4326,
            accuracyMeters = 4.5f,
            accelXyz = floatArrayOf(0f, -0.0f, 9.81f),
            gyroXyz = floatArrayOf(0f, 0f, -0.0f),
            moireScore = 0.05f,
            imageSha256 = digest,
        )
        val payloadPos = CanonicalPayload.buildCaptureSigningBytes(
            captureId = "cap-1",
            capturedAtUtcMs = 1_795_017_600_000L,
            sourceTag = CanonicalPayload.SOURCE_CAMERAX_LIVE,
            latitude = 0.0,
            longitude = 19.4326,
            accuracyMeters = 4.5f,
            accelXyz = floatArrayOf(0f, 0.0f, 9.81f),
            gyroXyz = floatArrayOf(0f, 0f, 0.0f),
            moireScore = 0.05f,
            imageSha256 = digest,
        )

        assertArrayEquals(payloadNeg, payloadPos)
        assertEquals("BIODEX_CAPTURE_V1", String(payloadNeg.copyOfRange(0, 17)))
    }

    @Test
    fun signsAndVerifiesWithEcdsaP256Sha256() {
        val kpg = KeyPairGenerator.getInstance("EC").apply {
            initialize(ECGenParameterSpec("secp256r1"))
        }
        val kp = kpg.generateKeyPair()
        val payload = CanonicalPayload.buildCaptureSigningBytes(
            captureId = "550e8400-e29b-41d4-a716-446655440000",
            capturedAtUtcMs = 1_795_017_600_000L,
            sourceTag = CanonicalPayload.SOURCE_CAMERAX_LIVE,
            latitude = 19.4326,
            longitude = -99.1332,
            accuracyMeters = 4.5f,
            accelXyz = floatArrayOf(0.12f, -0.05f, 9.79f),
            gyroXyz = floatArrayOf(0.01f, -0.02f, 0.0f),
            moireScore = 0.08f,
            imageSha256 = CanonicalPayload.sha256("mock-frame".toByteArray()),
        )

        val sigBytes = Signature.getInstance("SHA256withECDSA").run {
            initSign(kp.private)
            update(payload)
            sign()
        }

        val verified = Signature.getInstance("SHA256withECDSA").run {
            initVerify(kp.public)
            update(payload)
            verify(sigBytes)
        }
        assertTrue(verified)
    }
}
