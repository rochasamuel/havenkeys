package net.havenkeys.android.ui.shell

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
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

@OptIn(ExperimentalCoroutinesApi::class)
class ShellViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val vault = FakeVaultRepository()
    private val accounts = FakeAccountRepository()
    private val events = VaultEventsHub()

    private fun vm() = ShellViewModel(vault, accounts, events)

    private fun ShellViewModel.tiles() = state.value.addTiles.associate { it.tile to it.enabled }

    @Test
    fun offlineTheItemTilesAreDimmedButNotTheGenerator() {
        val vm = vm()
        assertEquals(
            mapOf(AddTile.LOGIN to false, AddTile.NOTE to false, AddTile.CARD to false, AddTile.GENERATOR to true),
            vm.tiles(),
        )
    }

    @Test
    fun onlineEveryTileIsOnAndOfflineAgainDimsThem() {
        val vm = vm()
        events.connectivity(true)
        assertTrue(vm.state.value.online)
        assertTrue(vm.tiles().values.all { it })
        events.connectivity(false)
        assertEquals(listOf(AddTile.GENERATOR), vm.tiles().filterValues { it }.keys.toList())
    }

    @Test
    fun theIdentityIsNeverATile() {
        assertTrue(AddTile.entries.none { it.kind == ItemKind.IDENTITY })
        assertEquals(
            listOf(ItemKind.LOGIN, ItemKind.SECURE_NOTE, ItemKind.CARD),
            AddTile.entries.mapNotNull { it.kind },
        )
    }

    @Test
    fun lockLocks() {
        vm().lock()
        assertEquals(listOf("lock"), vault.calls)
    }

    @Test
    fun syncNowSyncsOnce() {
        val vm = vm()
        vm.sync()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertFalse(vm.state.value.syncing)
        assertNull(vm.state.value.syncError)
    }

    @Test
    fun aFailedSyncIsReportedOnce() {
        accounts.sync = Outcome.Failed("offline")
        val vm = vm()
        vm.sync()
        assertEquals("offline", vm.state.value.syncError)
        vm.syncErrorShown()
        assertNull(vm.state.value.syncError)
    }

    @Test
    fun aLockDropsAPendingSyncError() {
        accounts.sync = Outcome.Failed("offline")
        val vm = vm()
        vm.sync()
        events.locked("user")
        assertNull(vm.state.value.syncError)
    }
}
