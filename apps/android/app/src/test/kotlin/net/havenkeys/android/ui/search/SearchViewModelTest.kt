package net.havenkeys.android.ui.search

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
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class SearchViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        searches += listOf("bank", "mail")
    }

    private fun vm() = SearchViewModel(vault, events)

    @Test
    fun itOpensOnTheRecentSearches() {
        val state = vm().state.value
        assertEquals("", state.query)
        assertEquals(listOf("bank", "mail"), state.recents)
        assertTrue(state.results.isEmpty())
    }

    @Test
    fun typingSearchesCountsAsActivityAndRecordsNothing() {
        val vm = vm()
        vm.setQuery("git")
        assertEquals(listOf(github), vm.state.value.results)
        assertTrue(vm.state.value.searched)
        assertTrue("touch" in vault.calls)
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun openingAResultRecordsTheQuery() {
        val vm = vm()
        vm.setQuery("git ")
        vm.opened()
        assertEquals(listOf("git", "bank", "mail"), vault.searches)
    }

    @Test
    fun openingWithABlankQueryRecordsNothing() {
        val vm = vm()
        vm.setQuery("   ")
        vm.opened()
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun clearingTheFieldBringsBackTheRecentsWithTheNewOne() {
        val vm = vm()
        vm.setQuery("git")
        vm.opened()
        vm.setQuery("")
        assertTrue(vm.state.value.results.isEmpty())
        assertFalse(vm.state.value.searched)
        assertEquals(listOf("git", "bank", "mail"), vm.state.value.recents)
    }

    @Test
    fun aRecentSearchRunsAgainWithoutBeingRecorded() {
        val vm = vm()
        vm.useRecent("mail")
        assertEquals("mail", vm.state.value.query)
        assertEquals(listOf(github), vm.state.value.results)
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun clearEmptiesTheRecents() {
        val vm = vm()
        vm.clearRecents()
        assertTrue(vm.state.value.recents.isEmpty())
        assertTrue(vault.searches.isEmpty())
    }

    @Test
    fun theSameQueryAgainDoesNothing() {
        val vm = vm()
        vm.setQuery("git")
        vault.calls.clear()
        vm.setQuery("git")
        assertTrue(vault.calls.isEmpty())
    }

    @Test
    fun anItemsChangedEventSearchesAgain() {
        val vm = vm()
        vm.setQuery("git")
        vault.items = Outcome.Ok(emptyList())
        events.itemsChanged()
        assertTrue(vm.state.value.results.isEmpty())
    }

    @Test
    fun aFailedSearchShowsItsCode() {
        val vm = vm()
        vault.items = Outcome.Failed("locked")
        vm.setQuery("git")
        assertEquals("locked", vm.state.value.errorCode)
    }

    @Test
    fun aLockWipesTheQueryResultsAndRecents() {
        val vm = vm()
        vm.setQuery("git")
        events.locked("user")
        assertEquals(SearchUiState(), vm.state.value)
        assertFalse(vm.state.value.toString().contains("git"))
    }

    @Test
    fun signingOutWipesThemToo() {
        val vm = vm()
        vm.setQuery("git")
        events.signedOut()
        assertEquals(SearchUiState(), vm.state.value)
    }
}
