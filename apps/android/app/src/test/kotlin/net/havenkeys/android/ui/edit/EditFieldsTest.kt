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
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextReplacement
import androidx.compose.ui.text.AnnotatedString
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

    private fun login(
        fields: List<EditField>,
        websites: List<Website> = emptyList(),
        tags: List<String> = emptyList(),
    ) = ItemEdit(ItemKind.LOGIN, "GitHub", websites, fields, false, true, 3L, tags = tags)

    private fun show(
        edit: ItemEdit,
        loading: List<String> = emptyList(),
        vaultTags: List<String> = emptyList(),
    ): Pair<EditorState, SnapshotStateList<String>> {
        vault.edit = Outcome.Ok(edit)
        val vm = EditViewModel(vault, FakeAccountRepository(), VaultEventsHub(), EditTarget.Existing("id"))
        val editor = EditorState(edit)
        val pending = mutableStateListOf<String>().apply { addAll(loading) }
        rule.setKit {
            Column(Modifier.verticalScroll(rememberScrollState())) {
                EditFields(editor, edit, FieldValues(vm, pending), vaultTags)
            }
        }
        return editor to pending
    }

    private fun typed(value: String) =
        SemanticsMatcher.expectValue(SemanticsProperties.EditableText, AnnotatedString(value))

    private fun field(label: String) = rule.onNode(hasSetTextAction() and hasText(label))

    @Test
    fun doneAddsTheTypedTagAndRemoveTakesItOff() {
        val (editor, _) = show(login(emptyList(), tags = listOf("work")))
        field(text(R.string.edit_add_tag)).performTextReplacement("  Staging ")
        field(text(R.string.edit_add_tag)).performImeAction()
        rule.runOnIdle { assertEquals(listOf("staging", "work"), editor.toDraft().tags) }
        field(text(R.string.edit_add_tag)).assert(typed(""))
        rule.onNodeWithContentDescription(text(R.string.edit_remove_tag, "work")).performClick()
        rule.runOnIdle {
            assertEquals(listOf("staging"), editor.toDraft().tags)
            assertTrue(editor.dirty)
        }
    }

    @Test
    fun aRefusedTagStaysInTheFieldAndSaysWhyUntilItChanges() {
        val (editor, _) = show(login(emptyList()))
        val long = "x".repeat(33)
        field(text(R.string.edit_add_tag)).performTextReplacement(long)
        field(text(R.string.edit_add_tag)).performImeAction()
        rule.runOnIdle { assertEquals(emptyList<String>(), editor.toDraft().tags) }
        field(text(R.string.edit_add_tag)).assert(typed(long))
        field(text(R.string.edit_add_tag))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.edit_tag_too_long)))
        field(text(R.string.edit_add_tag)).performTextReplacement("x".repeat(32))
        field(text(R.string.edit_add_tag)).assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Error))
    }

    @Test
    fun aControlCharacterIsNamedAsNotAllowed() {
        show(login(emptyList()))
        field(text(R.string.edit_add_tag)).performTextReplacement("a\u0007b")
        field(text(R.string.edit_add_tag)).performImeAction()
        field(text(R.string.edit_add_tag))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.edit_tag_not_allowed)))
    }

    @Test
    fun aTagTheItemHasClearsTheFieldQuietly() {
        val (editor, _) = show(login(emptyList(), tags = listOf("work")))
        field(text(R.string.edit_add_tag)).performTextReplacement("Work")
        field(text(R.string.edit_add_tag)).performImeAction()
        field(text(R.string.edit_add_tag)).assert(typed(""))
        field(text(R.string.edit_add_tag)).assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Error))
        rule.runOnIdle { assertEquals(listOf("work"), editor.toDraft().tags) }
    }

    @Test
    fun aCommaEndsATag() {
        val (editor, _) = show(login(emptyList()))
        field(text(R.string.edit_add_tag)).performTextReplacement("prod, ops,db")
        rule.runOnIdle { assertEquals(listOf("ops", "prod"), editor.toDraft().tags) }
        field(text(R.string.edit_add_tag)).assert(typed("db"))
    }

    /** Final review (tags): a refused part of a pasted list was dropped without a word. */
    @Test
    fun aRefusedPartOfACommaListStaysInTheFieldAndSaysWhy() {
        val (editor, _) = show(login(emptyList()))
        val long = "x".repeat(33)
        field(text(R.string.edit_add_tag)).performTextReplacement("prod,$long,db")
        rule.runOnIdle { assertEquals(listOf("prod"), editor.toDraft().tags) }
        field(text(R.string.edit_add_tag)).assert(typed("$long,db"))
        field(text(R.string.edit_add_tag))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.edit_tag_too_long)))
    }

    @Test
    fun typingOffersTheVaultsMatchingTagsAndATapAddsOne() {
        val (editor, _) = show(
            login(emptyList(), tags = listOf("work")),
            vaultTags = listOf("staging", "stock", "work"),
        )
        rule.onNode(hasText("staging") and hasClickAction()).assertDoesNotExist()
        field(text(R.string.edit_add_tag)).performTextReplacement("st")
        rule.onNode(hasText("stock") and hasClickAction()).assertExists()
        rule.onNode(hasText("staging") and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(listOf("staging", "work"), editor.toDraft().tags) }
        rule.onNode(hasText("stock") and hasClickAction()).assertDoesNotExist()
    }

    @Test
    fun atTwentyTagsTheFieldGivesWayToTheLimit() {
        show(login(emptyList(), tags = (1..20).map { "t%02d".format(it) }))
        field(text(R.string.edit_add_tag)).assertDoesNotExist()
        rule.onNodeWithText(text(R.string.edit_tag_limit)).assertExists()
    }

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
        rule.onNode(hasText(password) and hasContentDescription(text(R.string.hidden_value))).assertExists()
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
