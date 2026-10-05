package net.havenkeys.android.data

import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class VaultEventsHubTest {
    @Test
    fun lockAndUnlockDriveTheState() = runTest {
        val hub = VaultEventsHub()
        assertFalse(hub.unlocked.value)
        hub.unlocked()
        assertTrue(hub.unlocked.value)
        hub.locked("auto")
        assertFalse(hub.unlocked.value)
    }

    @Test
    fun eventsAreReplayedToALateCollectorOnlyOnce() = runTest {
        val hub = VaultEventsHub()
        hub.locked("bundle_refused")
        assertEquals(VaultEvent.Locked("bundle_refused"), hub.events.first())
    }

    @Test
    fun aDeletedAccountWipesLikeARemovalAndIsRemembered() = runTest {
        val hub = VaultEventsHub()
        hub.unlocked()
        hub.accountDeleted()
        assertFalse(hub.unlocked.value)
        assertTrue(hub.accountDeleted.value)
        assertEquals(VaultEvent.Removed, hub.events.first())
        hub.accountDeletedSeen()
        assertFalse(hub.accountDeleted.value)
    }
}
