package net.havenkeys.android.ui.pairing

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAccountRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.PairingRequestView

@OptIn(ExperimentalCoroutinesApi::class)
class PairingViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val link = "havenkeys://pair/v1?x"
    private val accounts = FakeAccountRepository()
    private val view = PairingRequestView(link, "Desktop · Linux", "187.1.2.3", "São Paulo, BR", "2026-10-03T12:00:00Z")

    private fun frame() = LumaFrame(1u, 1u, byteArrayOf(1))

    private fun scanned(): PairingViewModel {
        accounts.scannedLink = Outcome.Ok(link)
        accounts.request = Outcome.Ok(view)
        return PairingViewModel(accounts).also { it.onFrame(frame()) }
    }

    @Test
    fun aScannedCodeShowsTheRequest() = runTest {
        val vm = scanned()
        advanceUntilIdle()
        assertEquals(PairingUiState.Stage.CONFIRM, vm.state.value.stage)
        assertEquals("Desktop · Linux", vm.state.value.request?.deviceName)
    }

    @Test
    fun allowWithoutTheBiometricCheckApprovesNothing() = runTest {
        val vm = scanned()
        vm.allow(verify = { false })
        advanceUntilIdle()
        assertTrue(accounts.approved.isEmpty())
        assertEquals(PairingUiState.Stage.CONFIRM, vm.state.value.stage)
        vm.allow(verify = { true })
        advanceUntilIdle()
        assertEquals(listOf(link), accounts.approved)
        assertEquals(PairingUiState.Stage.DONE, vm.state.value.stage)
    }

    @Test
    fun denyDenies() = runTest {
        val vm = scanned()
        vm.deny()
        advanceUntilIdle()
        assertEquals(listOf(link), accounts.denied)
        assertEquals(PairingUiState.Stage.DENIED, vm.state.value.stage)
    }

    @Test
    fun aGoneCodeSaysSoAndScansAgain() = runTest {
        accounts.scannedLink = Outcome.Ok(link)
        accounts.request = Outcome.Failed("pairing_gone")
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame())
        advanceUntilIdle()
        assertEquals(PairingUiState.Stage.SCANNING, vm.state.value.stage)
        assertEquals("pairing_gone", vm.state.value.errorCode)
    }

    @Test
    fun aFrameWithNoPairingCodeIsDroppedQuietly() = runTest {
        val vm = PairingViewModel(accounts)
        vm.onFrame(frame())
        advanceUntilIdle()
        assertEquals(PairingUiState(), vm.state.value)
    }
}
