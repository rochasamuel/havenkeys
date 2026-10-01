package net.havenkeys.android.clipboard

import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.VaultEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipboardLockTest {
    private var clears = 0

    private suspend fun run(vararg events: VaultEvent) = clearClipboardOnLock(flowOf(*events)) { clears++ }

    @Test
    fun theLockSignOutAndRemovalClearTheClipboard() = runTest {
        run(VaultEvent.Locked("user"), VaultEvent.Locked("auto"), VaultEvent.SignedOut, VaultEvent.Removed)
        assertEquals(4, clears)
    }

    @Test
    fun aClipboardThatThrowsDoesNotEndTheCaller() = runTest {
        val clipboard = SensitiveClipboard({ throw SecurityException() }, backgroundScope)
        clipboard.clearIfOurs()
    }

    @Test
    fun ordinaryEventsDoNotClear() = runTest {
        run(VaultEvent.Unlocked, VaultEvent.Connectivity(online = true), VaultEvent.ItemsChanged)
        assertEquals(0, clears)
    }

    @Test
    fun ourClipIsCleared() {
        assertTrue(stillOurs(current = "t1", ours = "t1"))
    }

    @Test
    fun anUnreadableClipboardIsClearedWhileACopyOfOursIsOutstanding() {
        assertTrue(stillOurs(current = null, ours = "t1"))
    }

    @Test
    fun aReadableClipThatIsNotOursIsLeftAlone() {
        assertFalse(stillOurs(current = "other", ours = "t1"))
    }

    @Test
    fun nothingIsClearedWithoutACopyOfOurs() {
        assertFalse(stillOurs(current = null, ours = null))
        assertFalse(stillOurs(current = "t1", ours = null))
    }
}
