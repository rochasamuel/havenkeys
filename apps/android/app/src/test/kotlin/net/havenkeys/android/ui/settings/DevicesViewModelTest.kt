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
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.DeviceInfo

@OptIn(ExperimentalCoroutinesApi::class)
class DevicesViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val phone = DeviceInfo("1", "Pixel", "2026-09-01T10:00:00Z", null, true)
    private val laptop = DeviceInfo("2", "Laptop", "2026-08-01T10:00:00Z", "2026-09-30T10:00:00Z", false)
    private val accounts = FakeAccountRepository().apply { deviceList = Outcome.Ok(listOf(phone, laptop)) }
    private val events = VaultEventsHub()

    private fun vm() = DevicesViewModel(accounts, events)

    @Test
    fun listsTheDevices() = runTest {
        val state = vm().state.value
        assertEquals(listOf(phone, laptop), state.devices)
        assertNull(state.errorCode)
    }

    @Test
    fun offlineShowsTheOfflineCode() = runTest {
        accounts.deviceList = Outcome.Failed("offline")
        assertEquals("offline", vm().state.value.errorCode)
    }

    @Test
    fun reconnectingReloads() = runTest {
        accounts.deviceList = Outcome.Failed("offline")
        val vm = vm()
        accounts.deviceList = Outcome.Ok(listOf(phone))
        events.connectivity(true)
        assertEquals(listOf(phone), vm.state.value.devices)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun revokeCallsTheRepositoryAndReloads() = runTest {
        val vm = vm()
        accounts.deviceList = Outcome.Ok(listOf(phone))
        vm.revoke("2")
        assertEquals(listOf("devices", "revoke:2", "devices"), accounts.calls)
        assertEquals(listOf(phone), vm.state.value.devices)
    }

    @Test
    fun aFailedRevokeShowsItsCode() = runTest {
        val vm = vm()
        accounts.done = Outcome.Failed("offline")
        vm.revoke("2")
        assertEquals("offline", vm.state.value.errorCode)
    }
}
