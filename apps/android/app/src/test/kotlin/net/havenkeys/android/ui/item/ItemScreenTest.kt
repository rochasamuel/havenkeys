package net.havenkeys.android.ui.item

import android.content.ClipboardManager
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.bankRecord
import net.havenkeys.android.fakes.bankSummary
import uniffi.havenkeys_mobile.BankField
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.LifecycleRegistry
import androidx.lifecycle.compose.LocalLifecycleOwner
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField
import uniffi.havenkeys_mobile.ViewSection

/** Views here have no one-time code: CodeRow is tested alone (DetailRowsTest). */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class ItemScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()
    private fun text(id: Int, vararg args: Any) = app.getString(id, *args)

    private fun summary(kind: ItemKind = ItemKind.LOGIN, passkey: Boolean = false, tags: List<String> = emptyList()) =
        ItemSummary("id", kind, "GitHub", "sam", "github.com", false, passkey, 0, 0, tags = tags)

    private fun login(passkey: Boolean = false) = ItemView(
        summary(passkey = passkey),
        listOf(
            ViewField("username", "username", FieldKind.TEXT, "sam"),
            ViewField("password", "password", FieldKind.SECRET, null),
        ),
    )

    private val vault = FakeVaultRepository().apply {
        view = Outcome.Ok(login())
        revealed = Outcome.Ok("hunter2")
    }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))
    private val went = mutableListOf<String>()
    private val navigation = ItemNavigation(
        onBack = { went += "back" },
        onLock = { went += "lock" },
        onEdit = { went += "edit" },
        onTrashed = { went += "trashed:$it" },
        onDeleted = { went += "deleted:$it" },
    )

    private val hub = VaultEventsHub()

    private fun vm() = ItemViewModel(vault, FakeSettingsRepository(), hub, "id")

    private fun show(online: Boolean = true) {
        val vm = vm()
        rule.setKit { ItemScreen(vm, clipboard, online, navigation) }
    }

    @Test
    fun theTagsAreMarkersUnderTheTitle() {
        vault.view = Outcome.Ok(login().copy(summary = summary(tags = listOf("staging", "work"))))
        show()
        rule.onNodeWithContentDescription(text(R.string.item_tags)).assertExists()
        rule.onNodeWithText("staging").assert(hasClickAction().not())
        rule.onNodeWithText("work").assertExists()
    }

    @Test
    fun anUntaggedItemShowsNoTags() {
        show()
        rule.onNodeWithContentDescription(text(R.string.item_tags)).assertDoesNotExist()
    }

    @Test
    fun eachCustomSectionIsItsOwnGroupUnderItsTitle() {
        val bank = ViewSection(0u, "Bank")
        val untitled = ViewSection(1u, null)
        val fields = login().fields + listOf(
            ViewField("custom.a", "PIN", FieldKind.SECRET, null, bank),
            ViewField("custom.b", "Agency", FieldKind.SECRET, null, bank),
            ViewField("custom.c", "Host", FieldKind.SECRET, null, untitled),
            ViewField("notes", "notes", FieldKind.SECRET, null),
        )
        assertEquals(
            listOf(listOf("username", "password"), listOf("custom.a", "custom.b"), listOf("custom.c"), listOf("notes")),
            fieldGroups(fields).map { group -> group.map { it.key } },
        )
        vault.view = Outcome.Ok(login().copy(fields = fields))
        show()
        rule.onNode(hasText("Bank") and isHeading()).assertExists()
        rule.onNode(hasText("PIN")).assertExists()
    }

    @Test
    fun theTitleIsTheScreensHeadingAndTheUsernameOneStop() {
        show()
        rule.onNode(hasText("GitHub") and isHeading()).assertExists()
        rule.onNode(hasText(text(R.string.field_username)) and hasText("sam")).assertExists()
    }

    @Test
    fun aRevealedPasswordHidesItselfAfter30Seconds() {
        show()
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
        rule.mainClock.advanceTimeBy(RevealState.REVEAL_MS + 1)
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNode(hasText(text(R.string.field_password)) and hasContentDescription(text(R.string.hidden_value)))
            .assertExists()
    }

    @Test
    fun copyingThePasswordAsksRustCopiesAndCountsAUse() {
        show()
        rule.onNodeWithContentDescription(text(R.string.copy, text(R.string.field_password))).performClick()
        rule.waitForIdle()
        assertTrue("reveal:password" in vault.calls)
        assertEquals(listOf("id"), vault.usesRecorded)
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("hunter2", clip)
        rule.onNodeWithText(text(R.string.copied, text(R.string.field_password), 30)).assertExists()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }

    @Test
    fun aFailedRevealSaysWhyAndShowsNothing() {
        vault.revealed = Outcome.Failed("locked")
        show()
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText(text(errorText("locked"))).assertExists()
        rule.onNode(hasText(text(R.string.field_password)) and hasContentDescription(text(R.string.hidden_value)))
            .assertExists()
    }

    @Test
    fun deleteMovesTheItemToTheTrashWithoutAskingThenLeaves() {
        vault.view = Outcome.Ok(login(passkey = true))
        show()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).performClick()
        rule.waitForIdle()
        rule.onAllNodes(isDialog()).assertCountEquals(0)
        assertTrue("trash:id" in vault.calls)
        assertEquals(listOf("trashed:GitHub"), went)
    }

    @Test
    fun anItemDeletedForGoodLeavesSayingSo() {
        vault.trashed = Outcome.Ok(false)
        show()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).performClick()
        rule.waitForIdle()
        assertEquals(listOf("deleted:GitHub"), went)
    }

    @Test
    fun aFailedDeleteSaysWhyAndStays() {
        vault.trashed = Outcome.Failed("offline")
        show()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).performClick()
        rule.onNodeWithText(text(errorText("offline"))).assertExists()
        assertEquals(emptyList<String>(), went)
    }

    /** Final review (stage 4): the menu closed offline must stay closed when the server comes back. */
    @Test
    fun theMoreMenuStaysClosedWhenTheAppComesBackOnline() {
        val vm = vm()
        var online by mutableStateOf(true)
        rule.setKit { ItemScreen(vm, clipboard, online, navigation) }
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).assertExists()
        online = false
        rule.waitForIdle()
        online = true
        rule.waitForIdle()
        rule.onAllNodesWithText(text(R.string.item_delete)).assertCountEquals(0)
    }

    @Test
    fun offlineEditAndMoreAreDisabled() {
        show(online = false)
        rule.onNodeWithContentDescription(text(R.string.item_edit)).assertIsNotEnabled()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).assertIsNotEnabled()
    }

    @Test
    fun theMoreMenuClosesWhenTheAppGoesOffline() {
        vault.view = Outcome.Ok(login())
        val vm = vm()
        var online by mutableStateOf(true)
        rule.setKit { ItemScreen(vm, clipboard, online, navigation) }
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).assertExists()
        online = false
        rule.waitForIdle()
        rule.onAllNodesWithText(text(R.string.item_delete)).assertCountEquals(0)
    }

    @Test
    fun theIdentityHasNoDelete() {
        vault.view = Outcome.Ok(ItemView(summary(ItemKind.IDENTITY), emptyList()))
        show()
        rule.onAllNodesWithContentDescription(text(R.string.vault_more)).assertCountEquals(0)
    }

    @Test
    fun aRestoredItemScreenComesBackMasked() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { ItemScreen(vm, clipboard, true, navigation) } }
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
        restoration.emulateSavedInstanceStateRestore()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNode(hasText(text(R.string.field_password)) and hasContentDescription(text(R.string.hidden_value)))
            .assertExists()
    }

    private fun reveal() {
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
    }

    private fun assertMasked() {
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNode(hasText(text(R.string.field_password)) and hasContentDescription(text(R.string.hidden_value)))
            .assertExists()
    }

    private class TestOwner : LifecycleOwner {
        val registry = LifecycleRegistry.createUnsafe(this)
        override val lifecycle: Lifecycle get() = registry
    }

    private fun showWith(owner: TestOwner) {
        rule.runOnUiThread { owner.registry.currentState = Lifecycle.State.RESUMED }
        val vm = vm()
        rule.setKit {
            CompositionLocalProvider(LocalLifecycleOwner provides owner) {
                ItemScreen(vm, clipboard, true, navigation)
            }
        }
    }

    @Test
    fun aRevealedValueIsClearedWhenTheAppStops() {
        val owner = TestOwner()
        showWith(owner)
        reveal()
        rule.runOnUiThread { owner.registry.handleLifecycleEvent(Lifecycle.Event.ON_STOP) }
        rule.waitForIdle()
        assertMasked()
    }

    /** Final review (stage 4): Rust's answer arriving after ON_STOP showed the value on a stopped screen. */
    @Test
    fun aRevealAnsweredAfterTheAppStoppedIsNotShown() {
        val owner = TestOwner()
        val gate = CompletableDeferred<Unit>()
        vault.revealGate = gate
        showWith(owner)
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.waitForIdle()
        assertTrue("reveal:password" in vault.calls)
        rule.runOnUiThread { owner.registry.handleLifecycleEvent(Lifecycle.Event.ON_STOP) }
        rule.waitForIdle()
        gate.complete(Unit)
        rule.runOnUiThread { owner.registry.currentState = Lifecycle.State.RESUMED }
        rule.waitForIdle()
        assertMasked()
    }

    @Test
    fun aRevealedValueIsClearedWhenItsRowLeavesComposition() {
        show()
        reveal()
        // A reload without the password field removes its row; when the field
        // returns, its row starts masked: nothing was kept anywhere.
        vault.view = Outcome.Ok(ItemView(summary(), listOf(ViewField("username", "username", FieldKind.TEXT, "sam"))))
        hub.itemsChanged()
        rule.waitForIdle()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        vault.view = Outcome.Ok(login())
        hub.itemsChanged()
        rule.waitForIdle()
        assertMasked()
    }

    /** What it proves: the lock empties the screen's item, so the revealed row (and its value) goes. */
    @Test
    fun aLockEmptiesTheItemAndTheRevealedRowGoesWithIt() {
        show()
        reveal()
        hub.locked("manual")
        rule.waitForIdle()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }

    private fun showBank() {
        vault.view = Outcome.Ok(ItemView(bankSummary(), emptyList()))
        vault.bankAccount = Outcome.Ok(bankRecord())
        vault.bankValues += mapOf(
            BankField.ACCOUNT to "12345",
            BankField.ACCOUNT_DIGIT to "6",
            BankField.PASSWORD to "s3nha",
        )
        show()
    }

    private val account get() = text(R.string.field_bank_account)

    @Test
    fun aBankAccountShowsItsFieldsWithTheAccountMasked() {
        showBank()
        rule.onNode(hasText(text(R.string.field_bank_bank)) and hasText("Itaú")).assertExists()
        rule.onNode(hasText(text(R.string.field_bank_branch)) and hasText("1234-5")).assertExists()
        rule.onNode(hasText(text(R.string.field_bank_type)) and hasText(text(R.string.bank_type_checking)))
            .assertExists()
        rule.onNode(hasText(text(R.string.field_bank_holder)) and hasText("Maria Silva")).assertExists()
        rule.onNodeWithContentDescription(text(R.string.bank_account_ending, "45-6"), useUnmergedTree = true)
            .assertExists()
        rule.onNodeWithText("itau.com.br", substring = true).assertExists()
        rule.onAllNodesWithText("12345", substring = true).assertCountEquals(0)
        assertTrue(vault.calls.none { it.startsWith("revealBank") })
        // The account type has nothing to copy; the others do.
        rule.onNodeWithContentDescription(text(R.string.copy, text(R.string.field_bank_type))).assertDoesNotExist()
        rule.onNodeWithContentDescription(text(R.string.copy, text(R.string.field_bank_holder))).assertExists()
    }

    @Test
    fun theEyeShowsTheAccountWithItsDigitUntilTheLock() {
        showBank()
        rule.onNodeWithContentDescription(text(R.string.reveal, account)).performClick()
        rule.onNodeWithText("12345-6").assertExists()
        hub.locked("manual")
        rule.waitForIdle()
        rule.onAllNodesWithText("12345-6").assertCountEquals(0)
    }

    @Test
    fun copyingTheAccountCopiesItWithItsDigit() {
        showBank()
        rule.onNodeWithContentDescription(text(R.string.copy, account)).performClick()
        rule.waitForIdle()
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("12345-6", clip)
        assertEquals(listOf("id"), vault.usesRecorded)
    }

    @Test
    fun copyDetailsCopiesRustsTextInTheAppsLanguage() {
        vault.details = Outcome.Ok("Itaú\nBranch 1234-5")
        showBank()
        rule.onNodeWithText(text(R.string.bank_copy_details)).performClick()
        rule.waitForIdle()
        assertTrue("bankDetails:en" in vault.calls)
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("Itaú\nBranch 1234-5", clip)
        rule.onNodeWithText(text(R.string.copied, text(R.string.bank_details), 30)).assertExists()
    }
}
