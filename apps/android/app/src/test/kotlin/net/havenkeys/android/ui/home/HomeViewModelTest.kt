package net.havenkeys.android.ui.home

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
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

@OptIn(ExperimentalCoroutinesApi::class)
class HomeViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val login = item("1", ItemKind.LOGIN, "GitHub")
    private val note = item("2", ItemKind.SECURE_NOTE, "Wi-Fi")
    private val identity = item("9", ItemKind.IDENTITY, "Sam")

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(login, note, identity))
        recent = Outcome.Ok(listOf(note, login))
        frequent = Outcome.Ok(listOf(login))
        view = Outcome.Ok(identityView("identity.first_name", "identity.email", "identity.cpf"))
    }

    private fun vm() = HomeViewModel(vault, accounts, events)

    @Test
    fun nothingLoadsBeforeHomeShows() {
        val vm = vm()
        assertTrue(vm.state.value.loading)
        assertTrue(vault.calls.none { it.startsWith("recent") || it.startsWith("frequent") })
    }

    @Test
    fun shownLoadsTheIdentityAndBothListsOfSix() {
        val vm = vm()
        vm.shown()
        val state = vm.state.value
        assertEquals(listOf(note, login), state.recent)
        assertEquals(listOf(login), state.frequent)
        val parts = listOf(IdentityPart.NAME, IdentityPart.EMAIL, IdentityPart.DOCUMENTS)
        assertEquals(IdentityCard("9", "Sam", parts), state.identity)
        assertFalse(state.loading)
        assertTrue("recent:6" in vault.calls)
        assertTrue("frequent:6" in vault.calls)
    }

    @Test
    fun anIdentityWithNothingFilledHasNoParts() {
        vault.view = Outcome.Ok(identityView())
        val vm = vm()
        vm.shown()
        assertEquals(emptyList<IdentityPart>(), vm.state.value.identity?.parts)
    }

    @Test
    fun withoutAnIdentityThereIsNoCard() {
        vault.items = Outcome.Ok(listOf(login, note))
        val vm = vm()
        vm.shown()
        assertNull(vm.state.value.identity)
    }

    @Test
    fun theIdentityCardNeverHoldsAValue() {
        vault.view = Outcome.Ok(identityView("identity.first_name", value = "Samuel"))
        val vm = vm()
        vm.shown()
        assertFalse(vm.state.value.toString().contains("Samuel"))
        assertTrue("nothing is revealed for Home", vault.calls.none { it.startsWith("reveal") })
    }

    @Test
    fun showingAgainPicksUpANewUse() {
        val vm = vm()
        vm.shown()
        vault.frequent = Outcome.Ok(listOf(note, login))
        vm.shown()
        assertEquals(listOf(note, login), vm.state.value.frequent)
    }

    @Test
    fun aLockDropsTheActivityData() {
        val vm = vm()
        vm.shown()
        events.locked("user")
        assertEquals(HomeUiState(), vm.state.value)
    }

    @Test
    fun signingOutOrRemovingDropsItToo() {
        val vm = vm()
        vm.shown()
        events.signedOut()
        assertEquals(HomeUiState(), vm.state.value)
        vm.shown()
        events.removed()
        assertEquals(HomeUiState(), vm.state.value)
    }

    @Test
    fun anItemsChangedEventReloads() {
        val vm = vm()
        vm.shown()
        vault.recent = Outcome.Ok(listOf(login))
        events.itemsChanged()
        assertEquals(listOf(login), vm.state.value.recent)
    }

    @Test
    fun aFailedLoadShowsItsCodeAndKeepsWhatLoaded() {
        vault.frequent = Outcome.Failed("locked")
        val vm = vm()
        vm.shown()
        assertEquals("locked", vm.state.value.errorCode)
        assertEquals(listOf(note, login), vm.state.value.recent)
        assertTrue(vm.state.value.frequent.isEmpty())
    }

    @Test
    fun refreshSyncsThenReloads() = runTest {
        val vm = vm()
        vm.shown()
        vault.recent = Outcome.Ok(listOf(login))
        vm.refresh()
        assertEquals(listOf("syncNow"), accounts.calls)
        assertEquals(listOf(login), vm.state.value.recent)
        assertFalse(vm.state.value.refreshing)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aFailedRefreshShowsItsCodeAndKeepsTheLists() = runTest {
        val vm = vm()
        vm.shown()
        accounts.sync = Outcome.Failed("offline")
        vm.refresh()
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(listOf(note, login), vm.state.value.recent)
        assertFalse(vm.state.value.refreshing)
    }

    @Test
    fun partsFollowTheFieldNames() {
        val keys = listOf(
            "identity.last_name",
            "identity.city",
            "identity.gender",
            "identity.home_phone",
            "identity.company",
            "card.number",
        )
        assertEquals(
            listOf(IdentityPart.NAME, IdentityPart.PHONE, IdentityPart.ADDRESS, IdentityPart.WORK, IdentityPart.OTHER),
            IdentityPart.of(keys),
        )
        assertEquals(emptyList<IdentityPart>(), IdentityPart.of(emptyList()))
    }

    private fun identityView(vararg keys: String, value: String? = null) =
        ItemView(identity, keys.map { ViewField(it, it, FieldKind.TEXT, value) })

    private fun item(id: String, kind: ItemKind, title: String) =
        ItemSummary(id, kind, title, null, null, false, false, 0, 0)
}
