package net.havenkeys.android

import java.io.IOException
import java.security.GeneralSecurityException
import java.security.ProviderException
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.VaultEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class KeystoreWipeTest {
    private val deleted = mutableListOf<String>()

    private suspend fun run(vararg events: VaultEvent) = wipeKeysOnExit(
        flowOf(*events),
        deleteBiometric = { deleted += "biometric" },
        deleteSecretKey = { deleted += "secret_key" },
    )

    @Test
    fun removingTheVaultDropsBothKeys() = runTest {
        run(VaultEvent.Removed)
        assertEquals(listOf("biometric", "secret_key"), deleted)
    }

    @Test
    fun signingOutAndARefusedBundleDropOnlyTheBiometricKey() = runTest {
        run(VaultEvent.SignedOut, VaultEvent.Locked("bundle_refused"))
        assertEquals(listOf("biometric", "biometric"), deleted)
    }

    @Test
    fun aKeystoreThatThrowsAnswersTheFallback() {
        val failures = listOf(GeneralSecurityException(), IOException(), IllegalStateException(), ProviderException())
        for (failure in failures) {
            assertFalse(keystoreOr(false) { throw failure })
        }
        assertTrue(keystoreOr(false) { true })
    }

    @Test
    fun aFailedDeleteDoesNotStopTheCollector() = runTest {
        wipeKeysOnExit(
            flowOf(VaultEvent.SignedOut, VaultEvent.Removed),
            deleteBiometric = { keystoreOr(Unit) { throw ProviderException() } },
            deleteSecretKey = { deleted += "secret_key" },
        )
        assertEquals(listOf("secret_key"), deleted)
    }

    @Test
    fun ordinaryEventsKeepEverything() = runTest {
        run(
            VaultEvent.Locked("user"),
            VaultEvent.Locked("auto"),
            VaultEvent.Unlocked,
            VaultEvent.Connectivity(online = true),
            VaultEvent.ItemsChanged,
        )
        assertEquals(emptyList<String>(), deleted)
    }
}
