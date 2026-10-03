package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isNotEnabled
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
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.MatchKind
import uniffi.havenkeys_mobile.Website

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class EditFieldsTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private fun login(fields: List<EditField>, websites: List<Website> = emptyList()) =
        ItemEdit(ItemKind.LOGIN, "GitHub", websites, fields, false, true, 3L)

    private fun show(
        edit: ItemEdit,
        loading: List<String> = emptyList(),
    ): Pair<EditorState, SnapshotStateList<String>> {
        vault.edit = Outcome.Ok(edit)
        val vm = EditViewModel(vault, FakeAccountRepository(), VaultEventsHub(), EditTarget.Existing("id"))
        val editor = EditorState(edit)
        val pending = mutableStateListOf<String>().apply { addAll(loading) }
        rule.setKit {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                EditFields(editor, edit, FieldValues(vm, pending))
            }
        }
        return editor to pending
    }

    private fun field(label: String) = rule.onNode(hasSetTextAction() and hasText(label))

    @Test
    fun typingTheTitleChangesTheDraft() {
        val (editor, _) = show(login(emptyList()))
        field(text(R.string.edit_title)).performTextReplacement("GitHub work")
        rule.runOnIdle {
            assertEquals("GitHub work", editor.toDraft().title)
            assertTrue(editor.dirty)
        }
    }

    @Test
    fun aHiddenPasswordIsReadFromRustOnlyWhenTheUserChangesIt() {
        vault.revealed = Outcome.Ok("hunter2")
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, true, null))))
        val password = text(R.string.field_password)
        rule.onNodeWithContentDescription(text(R.string.hidden, password)).assertExists()
        assertTrue(vault.calls.none { it == "reveal:password" })
        rule.onNodeWithText(text(R.string.edit_change)).performClick()
        rule.waitForIdle()
        assertTrue("reveal:password" in vault.calls)
        field(password).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        rule.runOnIdle {
            assertEquals("hunter2", editor.shown("password"))
            assertFalse(editor.dirty)
        }
    }

    @Test
    fun generateFillsThePasswordAndShowsIt() {
        vault.generated = Outcome.Ok(Generated("Gen-3rated!", 120.0))
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, false, null))))
        rule.onNodeWithContentDescription(text(R.string.edit_generate)).performClick()
        rule.waitForIdle()
        rule.runOnIdle { assertEquals("Gen-3rated!", editor.shown("password")) }
        rule.onNodeWithContentDescription(text(R.string.hide, text(R.string.field_password))).assertExists()
    }

    @Test
    fun aFieldWaitingForItsValueTakesNoTypingThenShowsIt() {
        val username = text(R.string.field_username)
        val (editor, pending) = show(
            login(listOf(EditField("username", FieldKind.TEXT, true, null))),
            loading = listOf("username"),
        )
        // A disabled field takes no text action, so it is found by its label and its disabled state.
        rule.onNode(hasText(username) and isNotEnabled()).assertExists()
        rule.runOnIdle {
            editor.load("username", "sam")
            pending.remove("username")
        }
        field(username).assertIsEnabled()
        assertEquals(
            "sam",
            field(username).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text,
        )
        rule.runOnIdle { assertFalse(editor.dirty) }
    }

    @Test
    fun theWebsiteMatchIsPickedFromASheet() {
        val (editor, _) = show(login(emptyList(), listOf(Website("github.com", MatchKind.DOMAIN))))
        rule.onNode(hasText(text(R.string.edit_match)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.edit_match_exact)) and hasRole(Role.RadioButton)).performClick()
        rule.runOnIdle { assertEquals(MatchKind.EXACT, editor.websites.single().match) }
    }

    @Test
    fun aWebsiteCanBeAddedAndRemoved() {
        val (editor, _) = show(login(emptyList(), listOf(Website("github.com", MatchKind.DOMAIN))))
        rule.onNode(hasText(text(R.string.edit_add_website)) and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(2, editor.websites.size) }
        rule.onAllNodes(hasClickAction() and SemanticsMatcher.expectValue(
            SemanticsProperties.ContentDescription,
            listOf(text(R.string.edit_remove_website)),
        ))[0].performClick()
        rule.runOnIdle { assertEquals(listOf(""), editor.websites.map { it.url }) }
    }

    @Test
    fun aRemovedFieldShowsNoValueAndCanComeBack() {
        val (editor, _) = show(login(listOf(EditField("password", FieldKind.SECRET, true, null))))
        rule.onNodeWithText(text(R.string.edit_remove)).performClick()
        rule.onNodeWithText(text(R.string.edit_will_be_removed), substring = true).assertExists()
        rule.runOnIdle { assertTrue(editor.isRemoved("password")) }
        rule.onNodeWithText(text(R.string.edit_undo)).performClick()
        rule.runOnIdle { assertFalse(editor.isRemoved("password")) }
    }

    @Test
    fun theCodesSetupKeyIsTypedLikeAPassword() {
        show(login(listOf(EditField("totp", FieldKind.TOTP, false, null))))
        field(text(R.string.field_totp)).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
    }
}
