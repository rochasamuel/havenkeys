package net.havenkeys.android.ui.item

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.take
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.settings
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.TotpNow
import uniffi.havenkeys_mobile.ViewField
import org.junit.Assert.assertTrue

@OptIn(ExperimentalCoroutinesApi::class)
class ItemViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val events = VaultEventsHub()
    private val settings = FakeSettingsRepository()

    private fun vm(vault: FakeVaultRepository) = ItemViewModel(vault, settings, events, "id")

    private fun summary() = ItemSummary("id", ItemKind.LOGIN, "GitHub", "octo", "github.com", true, false, 0)

    private fun loginView() = ItemView(
        summary(),
        listOf(
            ViewField("username", "username", FieldKind.TEXT, "octo"),
            ViewField("password", "password", FieldKind.SECRET, null),
        ),
    )

    @Test
    fun theViewStateNeverHoldsARevealedValue() = runTest {
        val vault = FakeVaultRepository().apply {
            view = Outcome.Ok(ItemView(summary(), listOf(ViewField("password", "password", FieldKind.SECRET, null))))
            revealed = Outcome.Ok("hunter2")
        }
        val vm = vm(vault)
        assertEquals(Outcome.Ok("hunter2"), vm.reveal("password"))
        assertFalse(vm.state.value.toString().contains("hunter2"))
        assertEquals(listOf("reveal:password"), vault.calls)
    }

    @Test
    fun loadsTheOverview() = runTest {
        val vm = vm(FakeVaultRepository().apply { view = Outcome.Ok(loginView()) })
        assertEquals(loginView(), vm.state.value.view)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFailedLoadShowsItsCode() = runTest {
        val vm = vm(FakeVaultRepository().apply { view = Outcome.Failed("not_found") })
        assertNull(vm.state.value.view)
        assertEquals("not_found", vm.state.value.errorCode)
    }

    @Test
    fun aLockedEventClearsTheView() = runTest {
        val vm = vm(FakeVaultRepository().apply { view = Outcome.Ok(loginView()) })
        events.locked("user")
        assertNull(vm.state.value.view)
    }

    @Test
    fun anItemsChangedEventReloads() = runTest {
        val vault = FakeVaultRepository().apply { view = Outcome.Ok(loginView()) }
        val vm = vm(vault)
        val renamed = loginView().copy(summary = summary().copy(title = "GitHub work"))
        vault.view = Outcome.Ok(renamed)
        events.itemsChanged()
        assertEquals(renamed, vm.state.value.view)
    }

    @Test
    fun totpTicksPollOncePerSecond() = runTest {
        val vault = FakeVaultRepository().apply { totpNow = Outcome.Ok(TotpNow("123456", 30u, 12u)) }
        val vm = vm(vault)
        backgroundScope.launch { vm.totpTicks().collect {} }
        runCurrent()
        assertEquals(listOf("totp"), vault.calls)
        advanceTimeBy(2_500)
        assertEquals(listOf("totp", "totp", "totp"), vault.calls)
    }

    @Test
    fun totpTicksCarryFailuresToo() = runTest {
        val vault = FakeVaultRepository().apply { totpNow = Outcome.Failed("locked") }
        assertEquals(listOf(Outcome.Failed("locked")), vm(vault).totpTicks().take(1).toList())
    }

    @Test
    fun theClipboardDelayComesFromTheSettings() = runTest {
        settings.current = Outcome.Ok(settings().copy(clipboardClearSeconds = 90u))
        assertEquals(90, vm(FakeVaultRepository()).clipboardClearSeconds())
    }

    @Test
    fun anUnreadableSettingClearsTheClipboardAtTheDefault() = runTest {
        settings.current = Outcome.Failed("locked")
        assertEquals(30, vm(FakeVaultRepository()).clipboardClearSeconds())
    }

    @Test
    fun deleteAsksRustForThisItem() = runTest {
        val vault = FakeVaultRepository().apply { view = Outcome.Ok(loginView()) }
        assertEquals(Outcome.Ok(Unit), vm(vault).delete())
        assertTrue("delete:id" in vault.calls)
    }
}
