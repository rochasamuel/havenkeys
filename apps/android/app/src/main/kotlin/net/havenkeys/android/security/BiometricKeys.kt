package net.havenkeys.android.security

import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyPermanentlyInvalidatedException
import android.security.keystore.KeyProperties
import android.security.keystore.StrongBoxUnavailableException
import java.io.File
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The biometric unlock bundle (spec §5.2), sealed by a Keystore key that
 * every use must unlock with a BIOMETRIC_STRONG prompt bound to the cipher.
 */
class BiometricKeys(context: Context) {
    private val file = File(context.noBackupFilesDir, "unlock-bundle.bin")

    fun hasBundle(): Boolean = file.exists() && keyStore().containsAlias(ALIAS)

    fun encryptCipher(): Cipher {
        delete()
        val cipher = Cipher.getInstance(SecretKeyCipher.TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, generate())
        return cipher
    }

    /** Null when there is no bundle, or the key was invalidated (new enrollment). */
    fun decryptCipher(): Cipher? {
        if (!hasBundle()) return null
        val key = keyStore().getKey(ALIAS, null) as? SecretKey
        val iv = file.readBytes().takeIf { it.size > SecretKeyCipher.IV_LEN }?.copyOf(SecretKeyCipher.IV_LEN)
        val cipher = if (key != null && iv != null) initDecrypt(key, iv) else null
        if (cipher == null) delete()
        return cipher
    }

    private fun initDecrypt(key: SecretKey, iv: ByteArray): Cipher? = try {
        Cipher.getInstance(SecretKeyCipher.TRANSFORMATION).apply {
            init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(SecretKeyCipher.TAG_BITS, iv))
        }
    } catch (@Suppress("SwallowedException") e: KeyPermanentlyInvalidatedException) {
        null
    }

    /** `bundle` is zeroed whatever happens. */
    fun store(cipher: Cipher, bundle: ByteArray) {
        try {
            val sealed = cipher.iv + cipher.doFinal(bundle)
            val tmp = File(file.path + ".tmp")
            tmp.writeBytes(sealed)
            check(tmp.renameTo(file))
        } finally {
            bundle.fill(0)
        }
    }

    /** The caller hands the result straight to Rust, then zeroes it. */
    @Throws(GeneralSecurityException::class)
    fun open(cipher: Cipher): ByteArray {
        val sealed = file.readBytes()
        return cipher.doFinal(sealed, SecretKeyCipher.IV_LEN, sealed.size - SecretKeyCipher.IV_LEN)
    }

    fun delete() {
        file.delete()
        keyStore().deleteEntry(ALIAS)
    }

    private fun keyStore() = KeyStore.getInstance(SecretKeyCipher.ANDROID_KEYSTORE).apply { load(null) }

    private fun generate(): SecretKey = try {
        generate(strongBox = true)
    } catch (@Suppress("SwallowedException") e: StrongBoxUnavailableException) {
        generate(strongBox = false)
    }

    private fun generate(strongBox: Boolean): SecretKey {
        val spec = KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            .setKeySize(SecretKeyCipher.KEY_BITS)
            .setUserAuthenticationRequired(true)
            .setInvalidatedByBiometricEnrollment(true)
            .setUnlockedDeviceRequired(true)
            .setIsStrongBoxBacked(strongBox)
            .apply {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                    setUserAuthenticationParameters(0, KeyProperties.AUTH_BIOMETRIC_STRONG)
                } else {
                    // API 28–29: -1 means every use needs a biometric prompt.
                    @Suppress("DEPRECATION")
                    setUserAuthenticationValidityDurationSeconds(-1)
                }
            }
            .build()
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, SecretKeyCipher.ANDROID_KEYSTORE)
            .apply { init(spec) }
            .generateKey()
    }

    private companion object {
        const val ALIAS = "havenkeys.unlock"
    }
}
