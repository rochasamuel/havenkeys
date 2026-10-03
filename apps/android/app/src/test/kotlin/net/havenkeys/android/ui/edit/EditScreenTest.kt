package net.havenkeys.android.ui.edit

import android.view.KeyEvent
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextReplacement
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
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
import org.robolectric.shadows.ShadowDialog
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class EditScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val vault = FakeVaultRepository().apply {
        edit = Outcome.Ok(
            ItemEdit(
                ItemKind.LOGIN,
                "GitHub",
                emptyList(),
                listOf(EditField("username", FieldKind.TEXT, true, "sam")),
                false,
                true,
                3L,
            ),
        )
    }
    private val done = mutableListOf<String>()
    private var backs = 0
    private val navigation = EditNavigation(onDone = { done += it }, onBack = { backs++ }, onLock = {})

    private fun vm() = EditViewModel(vault, FakeAccountRepository(), VaultEventsHub(), EditTarget.Existing("id"))

    private fun show(online: Boolean = true) {
        val vm = vm()
        rule.setKit { EditScreen(vm, isNew = false, online = online, navigation = navigation) }
    }

    private fun title() = rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_title)))

    private fun pressBack() = rule.runOnIdle {
        val window = ShadowDialog.getLatestDialog().window!!
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
    }

    @Test
    fun savingSendsTheDraftAndGoesOn() {
        show()
        title().performTextReplacement("GitHub work")
        rule.onNodeWithText(text(R.string.edit_save)).assertIsEnabled().performClick()
        rule.waitForIdle()
        assertEquals("GitHub work", vault.drafts.single().title)
        assertEquals(listOf("id"), done)
    }

    @Test
    fun offlineTheSaveWaitsAndTheScreenSaysWhy() {
        show(online = false)
        rule.onNodeWithText(text(R.string.edit_save)).assertIsNotEnabled()
        rule.onNodeWithText(text(R.string.edit_offline)).assertExists()
    }

    @Test
    fun leavingWithChangesAsksFirst() {
        show()
        title().performTextReplacement("Changed")
        rule.onNodeWithContentDescription(text(R.string.item_back)).performClick()
        rule.onNodeWithText(text(R.string.edit_discard_changes)).assertExists()
        rule.onNodeWithText(text(R.string.edit_keep_editing)).performClick()
        assertEquals(0, backs)
        rule.onNodeWithContentDescription(text(R.string.item_back)).performClick()
        rule.onNodeWithText(text(R.string.edit_discard)).performClick()
        assertEquals(1, backs)
    }

    @Test
    fun backOnTheConflictLeavesReloadWorking() {
        vault.updated = Outcome.Failed("item_changed_elsewhere")
        show()
        title().performTextReplacement("Changed")
        rule.onNodeWithText(text(R.string.edit_save)).performClick()
        rule.onNodeWithText(text(R.string.error_item_changed_elsewhere)).assertExists()
        pressBack()
        rule.onNodeWithText(text(R.string.edit_reload)).assertIsEnabled().performClick()
        rule.waitForIdle()
        rule.onNodeWithText(text(R.string.error_item_changed_elsewhere)).assertDoesNotExist()
        assertTrue(vault.calls.count { it == "editable:id" } >= 2)
    }

    @Test
    fun aRestoredEditorBringsNothingTypedBack() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { EditScreen(vm, false, true, navigation) } }
        title().performTextReplacement("Typed before the restore")
        restoration.emulateSavedInstanceStateRestore()
        assertEquals("GitHub", title().fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text)
    }
}
