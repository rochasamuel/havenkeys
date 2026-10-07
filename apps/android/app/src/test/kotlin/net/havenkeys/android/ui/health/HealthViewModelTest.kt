package net.havenkeys.android.ui.health

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
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
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind
import uniffi.havenkeys_mobile.HealthView
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class HealthViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(summary("a", "Alpha"), summary("b", "Beta"), summary("c", "Gamma")))
        healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(1u, 2u, 0u, 1u, 0u, 0u, 0u),
                listOf(
                    HealthIssueView("a", listOf(HealthKind.WEAK, HealthKind.REUSED), 0u, null, false),
                    HealthIssueView("b", listOf(HealthKind.REUSED, HealthKind.PASSKEY), 0u, null, false),
                    HealthIssueView("c", listOf(HealthKind.OLD), null, null, true),
                ),
            ),
        )
    }

    private fun vm() = HealthViewModel(vault, events)

    @Test
    fun nothingLoadsBeforeTheScreenShows() {
        vm()
        assertFalse("health" in vault.calls)
    }

    @Test
    fun loadsRowsWithTitlesFromTheOverviews() {
        val vm = vm()
        vm.shown()
        val rows = vm.state.value.rows
        assertEquals(listOf("Alpha", "Beta"), rows.map { it.title })
        assertEquals(4, vm.state.value.total)
        assertFalse(vm.state.value.loading)
    }

    @Test
    fun filtersByKindAndShowsDismissed() {
        val vm = vm()
        vm.shown()
        vm.filter(HealthFilter.Kind(HealthKind.PASSKEY))
        assertEquals(listOf("Beta"), vm.state.value.rows.map { it.title })
        vm.filter(HealthFilter.Dismissed)
        assertEquals(listOf("Gamma"), vm.state.value.rows.map { it.title })
    }

    @Test
    fun dismissSendsTheFullListAndReloads() {
        val vm = vm()
        vm.shown()
        vm.dismiss("c", HealthKind.WEAK)
        // Sorted by the enum's order: WEAK before OLD.
        assertTrue("ignore:c:WEAK,OLD" in vault.calls)
        assertEquals(2, vault.calls.count { it == "health" })
    }

    @Test
    fun undoKeepsTheOtherDismissedChecks() {
        vault.healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(0u, 0u, 0u, 0u, 0u, 0u, 0u),
                listOf(HealthIssueView("c", listOf(HealthKind.WEAK, HealthKind.OLD), null, null, true)),
            ),
        )
        val vm = vm()
        vm.shown()
        vm.undo("c", HealthKind.OLD)
        assertTrue("ignore:c:WEAK" in vault.calls)
    }

    // The command replaces the login's whole list: a second Dismiss before the
    // report reloads must build on the first, not on the report that predates it.
    @Test
    fun twoDismissesOnOneLoginBeforeTheReloadKeepTheFirst() {
        val vm = vm()
        vm.shown()
        vault.healthGate = CompletableDeferred()
        vm.dismiss("a", HealthKind.WEAK)
        vm.dismiss("a", HealthKind.REUSED)
        assertEquals(listOf("ignore:a:WEAK", "ignore:a:WEAK,REUSED"), vault.calls.filter { it.startsWith("ignore") })
    }

    @Test
    fun aRowIsBusyOnlyWhileItsSaveIsInFlightAndHidesWhatWasDismissed() {
        val vm = vm()
        vm.shown()
        val saving = CompletableDeferred<Unit>()
        vault.ignoreGate = saving
        vault.healthGate = CompletableDeferred()
        vm.dismiss("a", HealthKind.WEAK)
        val pending = vm.state.value.rows.first { it.id == "a" }
        assertTrue(pending.busy)
        assertEquals(listOf(HealthKind.REUSED), pending.kinds)
        saving.complete(Unit)
        // Saved, the reload still running: the row is free and still hides the dismissed check.
        val saved = vm.state.value.rows.first { it.id == "a" }
        assertFalse(saved.busy)
        assertEquals(listOf(HealthKind.REUSED), saved.kinds)
    }

    @Test
    fun aFailedReloadAfterASaveLeavesTheRowFreeAndTheNextChangeBuildsOnTheSave() {
        val vm = vm()
        vm.shown()
        vault.healthView = Outcome.Failed("network")
        vm.dismiss("a", HealthKind.WEAK)
        assertFalse(vm.state.value.rows.first { it.id == "a" }.busy)
        vm.dismiss("a", HealthKind.REUSED)
        assertEquals(listOf("ignore:a:WEAK", "ignore:a:WEAK,REUSED"), vault.calls.filter { it.startsWith("ignore") })
    }

    @Test
    fun aFailedDismissSaysSoAndFreesTheRow() {
        vault.ignoreResult = Outcome.Failed("offline")
        val vm = vm()
        vm.shown()
        vm.dismiss("a", HealthKind.WEAK)
        assertEquals("offline", vm.state.value.errorCode)
        val row = vm.state.value.rows.first { it.id == "a" }
        assertFalse(row.busy)
        assertEquals(listOf(HealthKind.WEAK, HealthKind.REUSED), row.kinds)
    }

    @Test
    fun groupCountsAreClampedWhenTheOtherMembersAreNotShown() {
        vault.healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(0u, 1u, 0u, 0u, 0u, 0u, 1u),
                listOf(HealthIssueView("a", listOf(HealthKind.REUSED, HealthKind.DUPLICATE), 3u, 7u, false)),
            ),
        )
        val vm = vm()
        vm.shown()
        val row = vm.state.value.rows.single()
        assertEquals(2, row.groupSize)
        assertEquals(1, row.duplicates)
    }

    @Test
    fun groupCountsCountTheVisibleMembers() {
        val vm = vm()
        vm.shown()
        assertEquals(listOf(2, 2), vm.state.value.rows.map { it.groupSize })
    }

    @Test
    fun itemsChangedReloads() {
        val vm = vm()
        vm.shown()
        events.itemsChanged()
        assertEquals(2, vault.calls.count { it == "health" })
    }

    @Test
    fun theHelpLinkIsRustsOwnOrNothing() = runTest {
        val vm = vm()
        assertNull(vm.helpUrl("b", HealthKind.PASSKEY))
        vault.helpUrl = Outcome.Ok("https://example.com/passkeys")
        assertEquals("https://example.com/passkeys", vm.helpUrl("b", HealthKind.PASSKEY))
        assertTrue("help:b:PASSKEY" in vault.calls)
    }

    @Test
    fun lockWipesTheState() {
        val vm = vm()
        vm.shown()
        events.locked("user")
        assertTrue(vm.state.value.rows.isEmpty())
        assertNull(vm.state.value.counts)
    }

    private fun summary(id: String, title: String) =
        ItemSummary(id, ItemKind.LOGIN, title, null, null, false, false, 0, 0, tags = emptyList())
}
