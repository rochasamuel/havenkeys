package net.havenkeys.android.ui.vault

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
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

@OptIn(ExperimentalCoroutinesApi::class)
class VaultViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val login = item("1", ItemKind.LOGIN)
    private val passkeyLogin = item("2", ItemKind.LOGIN, hasPasskey = true)
    private val note = item("3", ItemKind.SECURE_NOTE)
    private val card = item("4", ItemKind.CARD)
    private val identity = item("5", ItemKind.IDENTITY)
    private val all = listOf(login, passkeyLogin, note, card, identity)

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply { items = Outcome.Ok(all) }

    private fun vm() = VaultViewModel(vault, accounts, events)

    @Test
    fun loadsTheItems() = runTest {
        assertEquals(all, vm().state.value.items)
    }

    @Test
    fun aFailedLoadShowsItsCode() = runTest {
        vault.items = Outcome.Failed("locked")
        val state = vm().state.value
        assertEquals("locked", state.errorCode)
        assertTrue(state.items.isEmpty())
    }

    @Test
    fun theFilterKeepsOnlyTheChosenKind() = runTest {
        val vm = vm()
        vm.setFilter(Filter.LOGINS)
        assertEquals(listOf(login, passkeyLogin), vm.state.value.items)
        vm.setFilter(Filter.NOTES)
        assertEquals(listOf(note), vm.state.value.items)
        vm.setFilter(Filter.CARDS)
        assertEquals(listOf(card), vm.state.value.items)
        vm.setFilter(Filter.IDENTITIES)
        assertEquals(listOf(identity), vm.state.value.items)
        vm.setFilter(Filter.PASSKEYS)
        assertEquals(listOf(passkeyLogin), vm.state.value.items)
        vm.setFilter(Filter.ALL)
        assertEquals(all, vm.state.value.items)
    }

    @Test
    fun aNonEmptyQueryCallsSearch() = runTest {
        val vm = vm()
        assertFalse("search" in vault.calls)
        vault.items = Outcome.Ok(listOf(note))
        vm.setQuery("git")
        assertEquals("git", vm.state.value.query)
        assertEquals(listOf("search"), vault.calls)
        assertEquals(listOf(note), vm.state.value.items)
    }

    @Test
    fun clearingTheQueryListsEverythingAgain() = runTest {
        val vm = vm()
        vm.setQuery("git")
        vm.setQuery("")
        assertEquals(listOf("search"), vault.calls)
        assertEquals(all, vm.state.value.items)
    }

    @Test
    fun theFilterAppliesToSearchResults() = runTest {
        val vm = vm()
        vm.setFilter(Filter.NOTES)
        vm.setQuery("x")
        assertEquals(listOf(note), vm.state.value.items)
    }

    @Test
    fun anItemsChangedEventReloads() = runTest {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(card))
        events.itemsChanged()
        assertEquals(listOf(card), vm.state.value.items)
    }

    @Test
    fun aLockedEventEmptiesTheList() = runTest {
        val vm = vm()
        events.locked("user")
        assertTrue(vm.state.value.items.isEmpty())
        assertFalse(vm.state.value.toString().contains(login.title))
    }

    @Test
    fun connectivityEventsUpdateOnline() = runTest {
        val vm = vm()
        assertFalse(vm.state.value.online)
        events.connectivity(true)
        assertTrue(vm.state.value.online)
        events.connectivity(false)
        assertFalse(vm.state.value.online)
    }

    @Test
    fun refreshSyncsAndReloads() = runTest {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(identity))
        vm.refresh()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertEquals(listOf(identity), vm.state.value.items)
        assertFalse(vm.state.value.refreshing)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFailedRefreshShowsItsCodeAndKeepsTheList() = runTest {
        val vm = vm()
        accounts.sync = Outcome.Failed("offline")
        vm.refresh()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(all, vm.state.value.items)
        assertFalse(vm.state.value.refreshing)
    }

    private fun item(id: String, kind: ItemKind, hasPasskey: Boolean = false) =
        ItemSummary(id, kind, "Title $id", null, null, false, hasPasskey, 0)
}
