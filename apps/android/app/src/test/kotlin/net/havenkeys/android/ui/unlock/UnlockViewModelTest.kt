package net.havenkeys.android.ui.unlock

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.status
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class UnlockViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private var deleted = 0
    private val unlockedStatus = status()

    private fun vm(vault: FakeVaultRepository, biometricAvailable: Boolean = true, hasBundle: Boolean = true) =
        UnlockViewModel(
            vault,
            biometricAvailable = biometricAvailable,
            hasBundle = { hasBundle },
            deleteBundle = { deleted++ },
        )

    @Test
    fun aWrongPasswordShowsUnlockFailed() = runTest {
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Failed("unlock_failed") })
        vm.unlockPassword("nope")
        assertEquals("unlock_failed", vm.state.value.errorCode)
        assertFalse(vm.state.value.unlocked)
        assertFalse(vm.state.value.busy)
    }

    @Test
    fun theRightPasswordUnlocks() = runTest {
        val vault = FakeVaultRepository()
        val vm = vm(vault)
        vm.unlockPassword("right")
        assertTrue(vm.state.value.unlocked)
        assertEquals(listOf("unlockPassword"), vault.calls)
    }

    @Test
    fun theBiometricButtonNeedsBothHardwareAndABundle() = runTest {
        assertTrue(vm(FakeVaultRepository(), biometricAvailable = true, hasBundle = true).state.value.offerBiometric)
        assertFalse(vm(FakeVaultRepository(), biometricAvailable = false, hasBundle = true).state.value.offerBiometric)
        assertFalse(vm(FakeVaultRepository(), biometricAvailable = true, hasBundle = false).state.value.offerBiometric)
    }

    @Test
    fun aRefusedBundleFallsBackToThePasswordAndIsDeleted() = runTest {
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Failed("bundle_refused") })
        vm.unlockWithBundle(byteArrayOf(1, 2, 3), 7)
        assertFalse(vm.state.value.offerBiometric)
        assertEquals("bundle_refused", vm.state.value.errorCode)
        assertEquals(1, deleted)
    }

    @Test
    fun anotherBundleFailureHidesTheButtonButKeepsTheBundle() = runTest {
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Failed("internal") })
        vm.unlockWithBundle(byteArrayOf(1, 2, 3), 7)
        assertFalse(vm.state.value.offerBiometric)
        assertEquals("internal", vm.state.value.errorCode)
        assertEquals(0, deleted)
    }

    @Test
    fun anUnusableBundleIsDeletedAndTheButtonHidden() = runTest {
        val vm = vm(FakeVaultRepository())
        vm.bundleUnusable()
        assertFalse(vm.state.value.offerBiometric)
        assertEquals(1, deleted)
    }

    @Test
    fun theBundleBytesAreZeroedAfterUse() = runTest {
        val bytes = byteArrayOf(1, 2, 3)
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Ok(unlockedStatus) })
        vm.unlockWithBundle(bytes, 7)
        assertArrayEquals(byteArrayOf(0, 0, 0), bytes)
        assertTrue(vm.state.value.unlocked)
    }

    @Test
    fun theBundleBytesAreZeroedWhenRefused() = runTest {
        val bytes = byteArrayOf(1, 2, 3)
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Failed("bundle_refused") })
        vm.unlockWithBundle(bytes, 7)
        assertArrayEquals(byteArrayOf(0, 0, 0), bytes)
    }

    @Test
    fun theStateNeverMentionsThePassword() = runTest {
        val vm = vm(FakeVaultRepository().apply { nextStatus = Outcome.Failed("unlock_failed") })
        vm.unlockPassword("correct horse battery")
        assertFalse(vm.state.value.toString().contains("correct horse"))
    }
}
