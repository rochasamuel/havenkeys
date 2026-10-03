package net.havenkeys.android.ui.item

import android.content.ClipboardManager
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
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
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
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

/** Views here have no one-time code: CodeRow is tested alone (DetailRowsTest). */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class ItemScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()
    private fun text(id: Int, vararg args: Any) = app.getString(id, *args)

    private fun summary(kind: ItemKind = ItemKind.LOGIN, passkey: Boolean = false) =
        ItemSummary("id", kind, "GitHub", "sam", "github.com", false, passkey, 0, 0)

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
        onDeleted = { went += "deleted" },
    )

    private val hub = VaultEventsHub()

    private fun vm() = ItemViewModel(vault, FakeSettingsRepository(), hub, "id")

    private fun show(online: Boolean = true) {
        val vm = vm()
        rule.setKit { ItemScreen(vm, clipboard, online, navigation) }
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
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
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
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }

    @Test
    fun deleteAsksFirstWarnsOfPasskeysAndLeavesAfterRust() {
        vault.view = Outcome.Ok(login(passkey = true))
        show()
        rule.onNodeWithContentDescription(text(R.string.vault_more)).performClick()
        rule.onNodeWithText(text(R.string.item_delete)).performClick()
        rule.onNodeWithText(text(R.string.item_confirm_delete, "GitHub")).assertExists()
        rule.onNodeWithText(text(R.string.item_passkey_warning)).assertExists()
        rule.onNode(hasText(text(R.string.item_delete)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertTrue("delete:id" in vault.calls)
        assertEquals(listOf("deleted"), went)
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
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }

    private fun reveal() {
        rule.onNodeWithContentDescription(text(R.string.reveal, text(R.string.field_password))).performClick()
        rule.onNodeWithText("hunter2").assertExists()
    }

    private fun assertMasked() {
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
        rule.onNodeWithContentDescription(text(R.string.hidden, text(R.string.field_password))).assertExists()
    }

    @Test
    fun aRevealedValueIsClearedWhenTheAppStops() {
        val owner = object : LifecycleOwner {
            val registry = LifecycleRegistry.createUnsafe(this)
            override val lifecycle: Lifecycle get() = registry
        }
        rule.runOnUiThread { owner.registry.currentState = Lifecycle.State.RESUMED }
        val vm = vm()
        rule.setKit {
            CompositionLocalProvider(LocalLifecycleOwner provides owner) {
                ItemScreen(vm, clipboard, true, navigation)
            }
        }
        reveal()
        rule.runOnUiThread { owner.registry.handleLifecycleEvent(Lifecycle.Event.ON_STOP) }
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

    @Test
    fun aRevealedValueIsClearedOnLock() {
        show()
        reveal()
        hub.locked("manual")
        rule.waitForIdle()
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }
}
