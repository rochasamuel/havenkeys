package net.havenkeys.android.security

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.havenkeys_mobile.CipherException
import uniffi.havenkeys_mobile.KeystoreCipher

/**
 * Seals the Secret Key file Rust writes (spec §4.1). No user authentication:
 * the Secret Key alone opens nothing, and the master password is still needed.
 */
// The Keystore's exception is dropped on purpose: its message may describe the key or data.
@Suppress("SwallowedException")
class SecretKeyCipher : KeystoreCipher {
    override fun seal(plaintext: ByteArray): ByteArray = try {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.iv + cipher.doFinal(plaintext)
    } catch (e: GeneralSecurityException) {
        throw CipherException.Failed()
    } finally {
        plaintext.fill(0)
    }

    override fun open(sealed: ByteArray): ByteArray = try {
        if (sealed.size <= IV_LEN) throw CipherException.Failed()
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, sealed, 0, IV_LEN))
        cipher.doFinal(sealed, IV_LEN, sealed.size - IV_LEN)
    } catch (e: GeneralSecurityException) {
        throw CipherException.Failed()
    }

    // One lock with `delete`: two first uses at once would otherwise each
    // generate a key, and a seal under the replaced one would never open.
    private fun key(): SecretKey = synchronized(Companion) {
        val store = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return@synchronized it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(KEY_BITS)
                .setUnlockedDeviceRequired(true)
                .build(),
        )
        generator.generateKey()
    }

    companion object {
        const val ALIAS = "havenkeys.secret_key"
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val IV_LEN = 12
        const val TAG_BITS = 128
        const val KEY_BITS = 256

        @Synchronized
        fun delete() {
            KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }.deleteEntry(ALIAS)
        }
    }
}
