package org.biodex.app.integrity

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.Signature
import java.security.spec.ECGenParameterSpec

data class SignedHardwarePayload(
    val publicKeySpkiDer: ByteArray,
    val signatureDer: ByteArray,
)

/**
 * Manages a device-bound ECDSA P-256 (`secp256r1`) keypair in Android Keystore
 * and signs canonical capture payloads with `SHA256withECDSA`.
 */
class HardwareKeystoreSigner(
    private val keyAlias: String = "biodex_capture_p256_v1",
) {
    private val keyStore: KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }

    fun ensureKeyPairExists(): KeyStore.PrivateKeyEntry {
        val existing = keyStore.getEntry(keyAlias, null) as? KeyStore.PrivateKeyEntry
        if (existing != null) {
            return existing
        }

        val generator = KeyPairGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_EC,
            ANDROID_KEYSTORE,
        )
        val spec = KeyGenParameterSpec.Builder(
            keyAlias,
            KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY,
        )
            .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
            .setDigests(KeyProperties.DIGEST_SHA256)
            .build()

        generator.initialize(spec)
        generator.generateKeyPair()

        return keyStore.getEntry(keyAlias, null) as KeyStore.PrivateKeyEntry
    }

    fun signPayload(canonicalBytes: ByteArray): SignedHardwarePayload {
        val entry = ensureKeyPairExists()
        val sig = Signature.getInstance("SHA256withECDSA").apply {
            initSign(entry.privateKey)
            update(canonicalBytes)
        }.sign()

        return SignedHardwarePayload(
            publicKeySpkiDer = entry.certificate.publicKey.encoded,
            signatureDer = sig,
        )
    }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
    }
}
