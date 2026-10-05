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
import org.junit.Assert.assertNull
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
    fun deletingTheAccountSignalsOnboarding() = runTest {
        val events = VaultEventsHub()
        val vm = RootViewModel(FakeVaultRepository(), events)
        events.accountDeleted()
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

    @Test
    fun afterAnUnlockTheResetKeepsTheVault() = runTest {
        val vault = FakeVaultRepository().apply { nextStatus = Outcome.Ok(status(LockState.LOCKED)) }
        val vm = RootViewModel(vault, VaultEventsHub())
        assertEquals(Start.UNLOCK, vm.start.value)
        vault.nextStatus = Outcome.Ok(status(LockState.UNLOCKED))
        assertEquals(Start.VAULT, vm.current())
        assertNull(routeToForce(Routes.SHELL, vm.current()))
    }

    @Test
    fun afterOnboardingTheResetDoesNotReturnToOnboarding() = runTest {
        val vault = FakeVaultRepository().apply { nextStatus = Outcome.Ok(status(LockState.LOCKED, exists = false)) }
        val vm = RootViewModel(vault, VaultEventsHub())
        assertEquals(Start.ONBOARDING, vm.start.value)
        vault.nextStatus = Outcome.Ok(status(LockState.UNLOCKED))
        assertNull(routeToForce(Routes.SHELL, vm.current()))
    }

    @Test
    fun aLockedVaultAtProcessStartStillResetsToUnlock() = runTest {
        val vault = FakeVaultRepository().apply { nextStatus = Outcome.Ok(status(LockState.LOCKED)) }
        val vm = RootViewModel(vault, VaultEventsHub())
        assertEquals(Routes.UNLOCK, routeToForce(Routes.ITEM, vm.current()))
    }
}
