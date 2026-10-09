package net.havenkeys.android.ui.settings

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.settings
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import net.havenkeys.android.ui.trash.trashSummary

@OptIn(ExperimentalCoroutinesApi::class)
class SettingsViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val repo = FakeSettingsRepository()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository()
    private var enrolled = false

    private val events = VaultEventsHub()

    private fun vm() = SettingsViewModel(repo, accounts, vault, events, biometricEnrolled = { enrolled })

    @Test
    fun loadsTheSettingsTheEmailAndTheBiometricState() = runTest {
        enrolled = true
        val state = vm().state.value
        assertEquals(settings(), state.settings)
        assertEquals("user@example.com", state.email)
        assertTrue(state.biometricEnrolled)
        assertNull(state.errorCode)
    }

    @Test
    fun aFailedLoadShowsItsCode() = runTest {
        repo.current = Outcome.Failed("locked")
        val state = vm().state.value
        assertNull(state.settings)
        assertEquals("locked", state.errorCode)
    }

    @Test
    fun eachSetterSendsTheFullUpdatedSettings() = runTest {
        val vm = vm()
        vm.setAutoLock(60u)
        vm.setClipboardSeconds(90u)
        vm.setLockOnScreenOff(false)
        vm.setConfirmBeforeFilling(true)
        vm.setAssetLinks(false)
        val base = settings()
        assertEquals(
            listOf(
                base.copy(autoLockMinutes = 60u),
                base.copy(autoLockMinutes = 60u, clipboardClearSeconds = 90u),
                base.copy(autoLockMinutes = 60u, clipboardClearSeconds = 90u, lockOnScreenOff = false),
                base.copy(
                    autoLockMinutes = 60u,
                    clipboardClearSeconds = 90u,
                    lockOnScreenOff = false,
                    confirmBeforeFilling = true,
                ),
                base.copy(
                    autoLockMinutes = 60u,
                    clipboardClearSeconds = 90u,
                    lockOnScreenOff = false,
                    confirmBeforeFilling = true,
                    assetLinks = false,
                ),
            ),
            repo.updates,
        )
        assertEquals(repo.updates.last(), vm.state.value.settings)
    }

    @Test
    fun aRefusedValueLeavesTheShownValueAndSetsTheCode() = runTest {
        val vm = vm()
        repo.updateResult = Outcome.Failed("invalid_input")
        vm.setAutoLock(7u)
        assertEquals(15u, vm.state.value.settings?.autoLockMinutes)
        assertEquals("invalid_input", vm.state.value.errorCode)
        repo.updateResult = Outcome.Ok(Unit)
        vm.setAutoLock(30u)
        assertEquals(30u, vm.state.value.settings?.autoLockMinutes)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun signOutCallsTheRepository() = runTest {
        vm().signOut()
        assertEquals(listOf("signOut"), accounts.calls)
    }

    @Test
    fun removeDevicePassesTheTypedConfirmationToRust() = runTest {
        val vm = vm()
        accounts.done = Outcome.Failed("invalid_input")
        vm.removeDevice("someone@example.com")
        assertEquals(listOf("removeDevice:someone@example.com"), accounts.calls)
        assertEquals("invalid_input", vm.state.value.removeErrorCode)
        assertFalse(vm.state.value.removing)
    }

    @Test
    fun deleteAccountPassesTheConfirmationAndCountsFailures() = runTest {
        val vm = vm()
        accounts.done = Outcome.Failed("unlock_failed")
        vm.deleteAccount("user@example.com", "wrong")
        assertEquals(listOf("deleteAccount:user@example.com"), accounts.calls)
        assertEquals("unlock_failed", vm.state.value.deleteErrorCode)
        assertEquals(1, vm.state.value.deleteFailures)
        assertFalse(vm.state.value.deleting)
        accounts.done = Outcome.Ok(Unit)
        vm.deleteAccount("user@example.com", "right")
        assertNull(vm.state.value.deleteErrorCode)
        assertEquals(1, vm.state.value.deleteFailures)
    }

    @Test
    fun aCancelledEnrollmentShowsNothingAndReadsOff() = runTest {
        enrolled = true
        val vm = vm()
        enrolled = false
        vm.biometricChanged(Outcome.Failed("cancelled"))
        assertFalse(vm.state.value.biometricEnrolled)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFailedEnrollmentShowsItsCode() = runTest {
        val vm = vm()
        vm.biometricChanged(Outcome.Failed("unlock_failed"))
        assertEquals("unlock_failed", vm.state.value.errorCode)
        assertFalse(vm.state.value.biometricEnrolled)
    }

    @Test
    fun turningItOffReadsOff() = runTest {
        enrolled = true
        val vm = vm()
        enrolled = false
        vm.biometricChanged()
        assertFalse(vm.state.value.biometricEnrolled)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFinishedEnrollmentReadsOn() = runTest {
        val vm = vm()
        enrolled = true
        vm.biometricChanged(Outcome.Ok(Unit))
        assertTrue(vm.state.value.biometricEnrolled)
    }

    @Test
    fun countsTheTrashAndRecountsWhenItemsChange() {
        vault.trashList = Outcome.Ok(listOf(trashSummary("a", "A")))
        val vm = vm()
        assertEquals(1, vm.state.value.trashCount)
        vault.trashList = Outcome.Ok(emptyList())
        events.itemsChanged()
        assertEquals(0, vm.state.value.trashCount)
    }

    @Test
    fun showsTheTrialAndTheFrozenState() = runTest {
        vault.nextStatus = Outcome.Ok(
            net.havenkeys.android.fakes.status(planStatus = "trialing", trialEndsAt = "2026-10-12T10:00:00Z"),
        )
        val vm = vm()
        assertEquals("trialing", vm.state.value.planStatus)
        assertEquals("2026-10-12T10:00:00Z", vm.state.value.trialEndsAt)
        assertFalse(vm.state.value.frozen)

        vault.nextStatus = Outcome.Ok(net.havenkeys.android.fakes.status(planStatus = "frozen", entitlement = "frozen"))
        events.planChanged()
        assertTrue(vm.state.value.frozen)
        assertEquals("frozen", vm.state.value.planStatus)
    }

    @Test
    fun theDaysLeftRoundUpAndStopAtZero() {
        val now = java.time.Instant.parse("2026-10-09T10:00:00Z")
        assertEquals(3, trialDaysLeft("2026-10-12T10:00:00Z", now))
        assertEquals(3, trialDaysLeft("2026-10-11T10:00:01Z", now))
        assertEquals(1, trialDaysLeft("2026-10-09T10:00:01Z", now))
        assertEquals(0, trialDaysLeft("2026-10-01T00:00:00Z", now))
        assertNull(trialDaysLeft("not a date", now))
    }
}
