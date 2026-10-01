package net.havenkeys.android.ui.nav

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.status
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.LockState
import uniffi.havenkeys_mobile.Status

@OptIn(ExperimentalCoroutinesApi::class)
class RootViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private fun startFor(next: Outcome<Status>): Start? {
        val vault = FakeVaultRepository().apply { nextStatus = next }
        return RootViewModel(vault, VaultEventsHub()).start.value
    }

    @Test
    fun withoutAVaultItStartsWithOnboarding() {
        assertEquals(Start.ONBOARDING, startFor(Outcome.Ok(status(LockState.LOCKED, exists = false))))
    }

    @Test
    fun aLockedVaultStartsWithUnlock() {
        assertEquals(Start.UNLOCK, startFor(Outcome.Ok(status(LockState.LOCKED))))
    }

    @Test
    fun anUnlockedVaultStartsWithTheVault() {
        assertEquals(Start.VAULT, startFor(Outcome.Ok(status(LockState.UNLOCKED))))
    }

    @Test
    fun anUnreadableStatusStartsWithUnlock() {
        assertEquals(Start.UNLOCK, startFor(Outcome.Failed("internal")))
    }

    @Test
    fun aLockEventSignalsTheUnlockScreen() = runTest {
        val events = VaultEventsHub()
        val vm = RootViewModel(FakeVaultRepository(), events)
        events.locked("user")
        assertEquals(Start.UNLOCK, vm.lockedSignal.first())
    }

    @Test
    fun removingTheVaultSignalsOnboarding() = runTest {
        val events = VaultEventsHub()
        val vm = RootViewModel(FakeVaultRepository(), events)
        events.removed()
        assertEquals(Start.ONBOARDING, vm.lockedSignal.first())
    }

    @Test
    fun otherEventsSignalNothing() = runTest {
        val events = VaultEventsHub()
        val vm = RootViewModel(FakeVaultRepository(), events)
        events.unlocked()
        events.itemsChanged()
        events.removed()
        assertEquals(Start.ONBOARDING, vm.lockedSignal.first())
    }
}
