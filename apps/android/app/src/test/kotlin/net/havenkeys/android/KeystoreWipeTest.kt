package net.havenkeys.android

import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.VaultEvent
import org.junit.Assert.assertEquals
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
