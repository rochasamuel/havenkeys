package net.havenkeys.android.ui.trash

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.TrashSummary

internal fun trashSummary(
    id: String,
    title: String,
    daysLeft: Int = 30,
    passkey: Boolean = false,
    trashedAt: Long = 0,
) =
    TrashSummary(
        ItemSummary(id, ItemKind.LOGIN, title, "sam", "example.com", false, passkey, 0, 0, tags = emptyList()),
        trashedAt,
        daysLeft.toUInt(),
    )

@OptIn(ExperimentalCoroutinesApi::class)
class TrashViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val events = VaultEventsHub()
    private val a = trashSummary("a", "Alpha", daysLeft = 28)
    private val b = trashSummary("b", "Beta", daysLeft = 30)
    private val vault = FakeVaultRepository().apply { trashList = Outcome.Ok(listOf(b, a)) }

    private fun vm() = TrashViewModel(vault, events)

    private fun listings() = vault.calls.count { it == "listTrash" }

    @Test
    fun loadsTheTrashNewestFirstAsGiven() {
        val state = vm().state.value
        assertEquals(listOf("b", "a"), state.rows.map { it.id })
        assertEquals(listOf(30, 28), state.rows.map { it.daysLeft })
        assertFalse(state.loading)
    }

    @Test
    fun restoreCallsTheRepositoryAndReloads() {
        val vm = vm()
        var done = false
        vm.restore("a") { done = true }
        assertEquals(listOf("listTrash", "restore:a", "listTrash"), vault.calls)
        assertTrue(done)
        assertFalse(vm.state.value.busy)
    }

    @Test
    fun purgeAndEmptyReloadAndReportErrorsByCode() {
        val vm = vm()
        vault.purged = Outcome.Failed("item_changed_elsewhere")
        var done = false
        vm.purge("a") { done = true }
        assertEquals("item_changed_elsewhere", vm.state.value.errorCode)
        assertFalse(done)
        assertEquals(listOf("listTrash", "purge:a", "listTrash"), vault.calls)

        vault.emptied = Outcome.Failed("offline")
        vm.emptyTrash()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(listOf("emptyTrash", "listTrash"), vault.calls.drop(3))

        vault.emptied = Outcome.Ok(2)
        vm.emptyTrash()
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun reloadsWhenTheVaultReportsItemsChanged() {
        vm()
        val before = listings()
        events.itemsChanged()
        assertEquals(before + 1, listings())
    }

    @Test
    fun aLockDropsTheList() {
        val vm = vm()
        events.locked("auto")
        assertEquals(TrashUiState(), vm.state.value)
    }

    @Test
    fun aFailedLoadSaysWhy() {
        vault.trashList = Outcome.Failed("locked")
        val state = vm().state.value
        assertEquals("locked", state.errorCode)
        assertFalse(state.loading)
    }

    @Test
    fun daysSinceCountsWholeDaysAndNeverGoesNegative() {
        val day = 86_400_000L
        assertEquals(0, daysSince(trashedAt = 1_000, now = 1_000 + day - 1))
        assertEquals(3, daysSince(trashedAt = 0, now = 3 * day + 5))
        assertEquals(0, daysSince(trashedAt = 10 * day, now = 0))
    }
}
