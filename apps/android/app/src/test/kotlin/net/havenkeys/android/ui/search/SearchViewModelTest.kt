package net.havenkeys.android.ui.search

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestCoroutineScheduler
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
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
    private val scheduler = TestCoroutineScheduler()

    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher(scheduler))

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
        scheduler.advanceUntilIdle()
        assertEquals(listOf(github), vm.state.value.results)
        assertTrue(vm.state.value.searched)
        assertTrue("touch" in vault.calls)
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun openingAResultRecordsTheQuery() {
        val vm = vm()
        vm.setQuery("git ")
        scheduler.advanceUntilIdle()
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
        scheduler.advanceUntilIdle()
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
        scheduler.advanceUntilIdle()
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
        scheduler.advanceUntilIdle()
        vault.calls.clear()
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        assertTrue(vault.calls.isEmpty())
    }

    @Test
    fun anItemsChangedEventSearchesAgain() {
        val vm = vm()
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        vault.items = Outcome.Ok(emptyList())
        events.itemsChanged()
        assertTrue(vm.state.value.results.isEmpty())
    }

    @Test
    fun aFailedSearchShowsItsCode() {
        val vm = vm()
        vault.items = Outcome.Failed("locked")
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        assertEquals("locked", vm.state.value.errorCode)
    }

    @Test
    fun aLockWipesTheQueryResultsAndRecents() {
        val vm = vm()
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        events.locked("user")
        assertEquals(SearchUiState(queryRevision = 1), vm.state.value)
        assertFalse(vm.state.value.toString().contains("git"))
    }

    @Test
    fun signingOutWipesThemToo() {
        val vm = vm()
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        events.signedOut()
        assertEquals(SearchUiState(queryRevision = 1), vm.state.value)
    }

    @Test
    fun aLockLandingBeforeTheRecentsLoadLeavesNothingBehind() {
        val gate = CompletableDeferred<Unit>()
        val gated = object : VaultRepository by vault {
            override suspend fun recentSearches(): Outcome<List<String>> {
                gate.await()
                return vault.recentSearches()
            }
        }
        val vm = SearchViewModel(gated, events)
        events.locked("user")
        gate.complete(Unit)
        assertEquals(SearchUiState(queryRevision = 1), vm.state.value)
    }

    @Test
    fun twoQuickKeystrokesRunOneSearchForTheLatter() {
        val vm = vm()
        vm.setQuery("g")
        scheduler.advanceTimeBy(50)
        vm.setQuery("gi")
        scheduler.advanceUntilIdle()
        assertEquals(1, vault.calls.count { it == "search" })
        assertEquals(1, vault.calls.count { it == "touch" })
        assertEquals("gi", vm.state.value.query)
        assertEquals(listOf(github), vm.state.value.results)
    }

    @Test
    fun openingRecordsTheQueryThatProducedTheResultsNotAPendingOne() {
        val vm = vm()
        vm.setQuery("git")
        scheduler.advanceUntilIdle()
        vm.setQuery("gith")
        vm.opened()
        assertEquals(listOf("git", "bank", "mail"), vault.searches)
    }
}
