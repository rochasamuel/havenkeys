package net.havenkeys.android.ui.items

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
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class ItemListViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val bank = item("1", ItemKind.LOGIN, "Banco")
    private val agua = item("2", ItemKind.LOGIN, "Água", passkey = true)
    private val amazon = item("3", ItemKind.SECURE_NOTE, "amazon")
    private val card = item("4", ItemKind.CARD, "Visa")
    private val identity = item("5", ItemKind.IDENTITY, "Sam")

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply { items = Outcome.Ok(listOf(bank, agua, amazon, card, identity)) }

    private fun vm() = ItemListViewModel(vault, accounts, events)

    @Test
    fun aToZIgnoresCaseAndAccents() {
        assertEquals(listOf("Água", "amazon", "Banco", "Sam", "Visa"), vm().state.value.items.map { it.title })
    }

    @Test
    fun eachCategoryIsCounted() {
        val counts = categoryCounts(vm().state.value.items)
        assertEquals(
            mapOf(
                Category.ALL to 5,
                Category.LOGINS to 2,
                Category.PASSKEYS to 1,
                Category.NOTES to 1,
                Category.CARDS to 1,
            ),
            counts,
        )
    }

    @Test
    fun aFailedLoadShowsItsCode() {
        vault.items = Outcome.Failed("locked")
        val state = vm().state.value
        assertEquals("locked", state.errorCode)
        assertTrue(state.items.isEmpty())
        assertFalse(state.loading)
    }

    @Test
    fun anItemsChangedEventReloads() {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(card))
        events.itemsChanged()
        assertEquals(listOf(card), vm.state.value.items)
    }

    @Test
    fun aLockEmptiesTheList() {
        val vm = vm()
        events.locked("user")
        assertEquals(ItemListUiState(), vm.state.value)
        assertFalse(vm.state.value.toString().contains("Banco"))
    }

    @Test
    fun refreshSyncsAndReloads() = runTest {
        val vm = vm()
        vault.items = Outcome.Ok(listOf(identity))
        vm.refresh()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertEquals(listOf(identity), vm.state.value.items)
        assertFalse(vm.state.value.refreshing)
    }

    @Test
    fun aFailedRefreshShowsItsCodeAndKeepsTheList() = runTest {
        val vm = vm()
        accounts.sync = Outcome.Failed("offline")
        vm.refresh()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(5, vm.state.value.items.size)
    }

    @Test
    fun tagCountsCountEachTagAToZ() {
        val items = listOf(
            item("1", ItemKind.LOGIN, "A", tags = listOf("prod", "work")),
            item("2", ItemKind.LOGIN, "B", tags = listOf("work")),
            item("3", ItemKind.LOGIN, "C"),
        )
        assertEquals(listOf("prod" to 1, "work" to 2), tagCounts(items))
    }

    private fun item(
        id: String,
        kind: ItemKind,
        title: String,
        passkey: Boolean = false,
        tags: List<String> = emptyList(),
    ) = ItemSummary(id, kind, title, null, null, false, passkey, 0, 0, tags = tags)
}
