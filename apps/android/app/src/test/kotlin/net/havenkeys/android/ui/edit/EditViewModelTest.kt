package net.havenkeys.android.ui.edit

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.bankRecord
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.BankField
import uniffi.havenkeys_mobile.Change
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldChange
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@OptIn(ExperimentalCoroutinesApi::class)
class EditViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val events = VaultEventsHub()
    private val accounts = FakeAccountRepository()
    private val edit = ItemEdit(
        ItemKind.LOGIN, "GitHub", emptyList(),
        listOf(EditField("password", FieldKind.SECRET, true, null)), false, true, 3L,
        tags = emptyList(),
    )
    private val draft = ItemDraft(
        ItemKind.LOGIN, "GitHub", emptyList(),
        listOf(FieldChange("password", Change.Replace("hunter3"))), 3L,
        tags = emptyList(),
    )

    private fun vault() = FakeVaultRepository().apply { this.edit = Outcome.Ok(this@EditViewModelTest.edit) }

    @Test
    fun opensTheItemAndHoldsNoValue() = runTest {
        val vault = vault().apply { revealed = Outcome.Ok("hunter2") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        assertEquals(edit, vm.state.value.edit)
        assertEquals(Outcome.Ok("hunter2"), vm.reveal("password"))
        assertFalse(vm.state.value.toString().contains("hunter2"))
        assertTrue("editable:id" in vault.calls)
    }

    private fun summary(id: String, tags: List<String>) =
        ItemSummary(id, ItemKind.LOGIN, id, null, null, false, false, 0, 0, tags = tags)

    /** Final review (tags): suggestions sorted by UTF-16 put "école" after "zoo"; now most used leads. */
    @Test
    fun theVaultsTagsAreSuggestedAsAReaderSortsThem() = runTest {
        val vault = vault().apply {
            items = Outcome.Ok(
                listOf(
                    summary("1", listOf("zoo", "école")),
                    summary("2", listOf("emploi", "zoo")),
                ),
            )
        }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        // Most used first ("zoo" twice), then as a reader sorts them.
        assertEquals(listOf("zoo", "école", "emploi"), vm.state.value.vaultTags)
    }

    /** One spelling per tag, most used first; the item's own tags are not the vault's (Rust leaves them out). */
    @Test
    fun theVaultsTagsMergeCaseAndLeaveTheItemOut() = runTest {
        val vault = vault().apply {
            items = Outcome.Ok(
                listOf(
                    summary("1", listOf("Work")),
                    summary("2", listOf("work", "prod")),
                    summary("id", listOf("Solo", "prod")),
                ),
            )
        }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        assertEquals(listOf("Work", "prod"), vm.state.value.vaultTags)
        events.locked("idle")
        assertEquals(emptyList<String>(), vm.state.value.vaultTags)
    }

    @Test
    fun ofSeveralSpellingsTheOneMostItemsCarryNamesTheTag() {
        val items = listOf(
            summary("1", listOf("work")),
            summary("2", listOf("Work")),
            summary("3", listOf("Work")),
            summary("4", listOf("prod")),
            summary("5", listOf("Prod")),
        )
        // "Work" 2 to 1; "Prod"/"prod" tie, so the smallest ("Prod") wins.
        assertEquals(listOf("Work", "Prod"), tagsByUse(items))
    }

    @Test
    fun theStateNamesNoUsername() = runTest {
        val withUsername = edit.copy(fields = listOf(EditField("username", FieldKind.TEXT, true, "octo")))
        val vault = FakeVaultRepository().apply { this.edit = Outcome.Ok(withUsername) }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        assertEquals(withUsername, vm.state.value.edit)
        assertFalse(vm.state.value.toString().contains("octo"))
    }

    @Test
    fun aNewItemStartsFromItsTemplate() = runTest {
        val vault = vault()
        EditViewModel(vault, accounts, events, EditTarget.New(ItemKind.CARD))
        assertTrue("template:CARD" in vault.calls)
    }

    @Test
    fun savingANewItemReportsItsId() = runTest {
        val vault = vault().apply { created = Outcome.Ok("abc") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.New(ItemKind.LOGIN))
        vm.save(draft)
        assertEquals(EditResult.Saved("abc"), vm.results.first())
        assertFalse(vm.state.value.saving)
        assertFalse("the draft is not kept", vm.state.value.toString().contains("hunter3"))
    }

    @Test
    fun savingAnEditReportsTheSameId() = runTest {
        val vault = vault()
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        vm.save(draft)
        assertEquals(EditResult.Saved("id"), vm.results.first())
        assertTrue("update:id" in vault.calls)
    }

    @Test
    fun aRefusedSaveShowsItsCodeAndKeepsTheEditorOpen() = runTest {
        val vault = vault().apply { updated = Outcome.Failed("offline") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        vm.save(draft)
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(edit, vm.state.value.edit)
        assertEquals(0, vm.state.value.generation)
    }

    @Test
    fun aConflictSyncsAndAsksToReload() = runTest {
        val vault = vault().apply { updated = Outcome.Failed("item_changed_elsewhere") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        vm.save(draft)
        assertTrue(vm.state.value.conflict)
        assertTrue("syncNow" in accounts.calls)
        assertEquals(listOf(true), accounts.freshCalls)
        vm.reload()
        assertFalse(vm.state.value.conflict)
        assertEquals(1, vm.state.value.generation)
        assertEquals(2, vault.calls.count { it == "editable:id" })
    }

    @Test
    fun aLockClearsTheEditor() = runTest {
        val vm = EditViewModel(vault(), accounts, events, EditTarget.Existing("id"))
        events.locked("idle")
        assertNull(vm.state.value.edit)
    }

    private val bankEdit =
        ItemEdit(ItemKind.BANK_ACCOUNT, "Conta Itaú", emptyList(), emptyList(), false, true, 4L, tags = emptyList())

    private fun bankVault() = FakeVaultRepository().apply {
        edit = Outcome.Ok(bankEdit)
        bankAccount = Outcome.Ok(bankRecord())
        bankValues[BankField.PASSWORD] = "s3nha"
    }

    @Test
    fun aBankAccountOpensItsBankEditor() = runTest {
        val vault = bankVault()
        val state = EditViewModel(vault, accounts, events, EditTarget.Existing("id")).state.value
        assertEquals(bankRecord(), state.bank)
        assertTrue("bankView:id" in vault.calls)
        // The password and notes are edited as a login's: hidden until Change.
        assertEquals(
            listOf(
                EditField("password", FieldKind.SECRET, true, null),
                EditField("notes", FieldKind.SECRET, false, null),
            ),
            state.edit?.fields,
        )
        assertFalse(state.toString().contains("Maria"))
    }

    @Test
    fun aNewBankAccountStartsEmpty() = runTest {
        val vault = bankVault()
        val state = EditViewModel(vault, accounts, events, EditTarget.New(ItemKind.BANK_ACCOUNT)).state.value
        assertTrue("template:BANK_ACCOUNT" in vault.calls)
        assertFalse(vault.calls.any { it.startsWith("bankView") })
        assertNull(state.bank)
        assertEquals(listOf(false, false), state.edit?.fields?.map { it.present })
    }

    @Test
    fun aBankAccountsPasswordIsReadAsABankValue() = runTest {
        val vault = bankVault()
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        assertEquals(Outcome.Ok("s3nha"), vm.reveal("password"))
        assertEquals(Outcome.Failed("not_found"), vm.reveal("notes"))
        assertEquals(listOf("revealBank:PASSWORD", "revealBank:NOTES"), vault.calls.filter { "reveal" in it })
    }

    @Test
    fun aBankAccountSavesThroughSaveBank() = runTest {
        val vault = bankVault().apply { bankSaved = Outcome.Ok("id") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        val draft = EditorState(vm.state.value.edit!!, bank = vm.state.value.bank).toBankDraft()
        vm.saveBank(draft)
        assertEquals(EditResult.Saved("id"), vm.results.first())
        assertEquals(listOf(draft), vault.bankDrafts)
        assertFalse("update:id" in vault.calls)
    }

    @Test
    fun aRefusedBankSaveKeepsTheEditorOpen() = runTest {
        val vault = bankVault().apply { bankSaved = Outcome.Failed("invalid_input") }
        val vm = EditViewModel(vault, accounts, events, EditTarget.Existing("id"))
        vm.saveBank(EditorState(vm.state.value.edit!!, bank = vm.state.value.bank).toBankDraft())
        assertEquals("invalid_input", vm.state.value.errorCode)
        assertEquals(bankRecord(), vm.state.value.bank)
    }

    @Test
    fun aLockClearsTheBankEditor() = runTest {
        val vm = EditViewModel(bankVault(), accounts, events, EditTarget.Existing("id"))
        events.locked("idle")
        assertNull(vm.state.value.bank)
    }
}
