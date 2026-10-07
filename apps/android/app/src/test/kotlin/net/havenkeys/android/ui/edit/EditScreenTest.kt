package net.havenkeys.android.ui.edit

import android.view.KeyEvent
import kotlin.math.abs
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
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
import uniffi.havenkeys_mobile.ItemSummary

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
                tags = emptyList(),
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

    /** Review (stage 4): Save sat 12dp from the edge, 4dp outside the fields' 16dp gutter. */
    @Test
    fun saveEndsOnTheSameGutterAsTheFields() {
        show()
        val save = rule.onNode(hasText(text(R.string.edit_save)) and hasClickAction()).fetchSemanticsNode().boundsInRoot
        val field = title().fetchSemanticsNode().boundsInRoot
        val density = rule.density.density
        assertTrue("save ends at ${save.right}, field at ${field.right}", abs(save.right - field.right) < density)
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
    fun aTagStillBeingTypedIsSavedWithTheDraft() {
        show()
        val tag = rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_add_tag)))
        tag.performClick()
        tag.performTextReplacement("Work")
        rule.onNodeWithText(text(R.string.edit_save)).assertIsEnabled().performClick()
        rule.waitForIdle()
        assertEquals(listOf("work"), vault.drafts.single().tags)
    }

    /** Final review (tags): Save with a refused tag in the field saved the item without it. */
    @Test
    fun saveWaitsWhileTheTagFieldHoldsATagRustWouldRefuse() {
        show()
        val tag = rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_add_tag)))
        tag.performClick()
        tag.performTextReplacement("x".repeat(33))
        rule.onNodeWithText(text(R.string.edit_save)).assertIsEnabled().performClick()
        rule.waitForIdle()
        assertTrue(vault.drafts.isEmpty())
        assertTrue(done.isEmpty())
        tag.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.edit_tag_too_long)))
    }

    /** Final review (tags): text typed in the tag field did not count as a change, so Back dropped it. */
    @Test
    fun aTagStillBeingTypedMakesBackAskFirst() {
        show()
        rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_add_tag))).performTextReplacement("work")
        rule.onNodeWithContentDescription(text(R.string.item_back)).performClick()
        rule.onNodeWithText(text(R.string.edit_discard_changes)).assertExists()
        assertEquals(0, backs)
    }

    @Test
    fun theVaultsTagsAreOfferedWhileTyping() {
        vault.items = Outcome.Ok(
            listOf(ItemSummary("2", ItemKind.LOGIN, "Bank", null, null, false, false, 0, 0, tags = listOf("staging"))),
        )
        show()
        rule.onNode(hasSetTextAction() and hasText(text(R.string.edit_add_tag))).performTextReplacement("sta")
        rule.onNode(hasText("staging") and hasClickAction()).assertExists()
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

    /** Final review (stage 4): the restore test now types a secret too, and it does not come back. */
    @Test
    fun aRestoredEditorBringsNoTypedPasswordBack() {
        vault.edit = Outcome.Ok(
            ItemEdit(
                ItemKind.LOGIN,
                "GitHub",
                emptyList(),
                listOf(
                    EditField("username", FieldKind.TEXT, true, "sam"),
                    EditField("password", FieldKind.SECRET, false, null),
                ),
                false,
                true,
                3L,
                tags = emptyList(),
            ),
        )
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { EditScreen(vm, false, true, navigation) } }
        val password = rule.onNode(hasSetTextAction() and hasText(text(R.string.field_password)))
        password.performTextInput("hunter2")
        assertEquals(7, password.fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.length)
        restoration.emulateSavedInstanceStateRestore()
        val after = rule.onNode(hasSetTextAction() and hasText(text(R.string.field_password)))
            .fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text.orEmpty()
        assertTrue("the typed password came back", after.isEmpty())
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }
}
