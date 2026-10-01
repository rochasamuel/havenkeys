package net.havenkeys.android.security

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.havenkeys_mobile.CipherException

@RunWith(AndroidJUnit4::class)
class KeystoreTest {
    @Test
    fun theSecretKeyCipherRoundTripsAndIsNotThePlaintext() {
        val c = SecretKeyCipher()
        val sealed = c.seal("A3-SECRET".toByteArray())
        assertFalse(String(sealed).contains("A3-SECRET"))
        assertArrayEquals("A3-SECRET".toByteArray(), c.open(sealed))
    }

    @Test(expected = CipherException.Failed::class)
    fun aTamperedSealIsRefused() {
        val c = SecretKeyCipher()
        val sealed = c.seal("x".toByteArray())
        sealed[sealed.size - 1] = (sealed.last().toInt() xor 1).toByte()
        c.open(sealed)
    }

    @Test
    fun withoutABundleThereIsNothingToDecrypt() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val keys = BiometricKeys(context)
        keys.delete()
        assertFalse(keys.hasBundle())
        assertNull(keys.decryptCipher())
    }

    @Test
    fun theBootCountIsKnown() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        assertTrue(BootCount.current(context) >= 0)
    }
}
