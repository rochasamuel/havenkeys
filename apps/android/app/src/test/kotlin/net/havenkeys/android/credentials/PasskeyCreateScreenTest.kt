package net.havenkeys.android.credentials

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.SHEET_TAG
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.AutofillMatch

@RunWith(RobolectricTestRunner::class)
class PasskeyCreateScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val calls = mutableListOf<String>()

    private fun state(homes: Int = 1, excluded: Boolean = false) = PasskeyCreateUiState(
        loading = false,
        rpId = "github.com",
        userName = "sam",
        homes = (1..homes).map { AutofillMatch("h$it", "Login $it", "user$it", false) },
        selected = "h1",
        excluded = excluded,
    )

    private fun show(state: PasskeyCreateUiState, fontScale: Float = 1f) = rule.setKit(fontScale = fontScale) {
        PasskeyCreateScreen(
            state = state,
            onSelect = { calls += "select:$it" },
            onSave = { calls += "save" },
            onCancel = { calls += "cancel" },
            onClose = { calls += "close" },
        )
    }

    @Test
    fun theLoginsAreRadioButtonsWithTheChosenOneSelected() {
        show(state())
        rule.onNode(hasText("Login 1") and hasRole(Role.RadioButton)).assertIsSelected()
        rule.onNode(hasText(text(R.string.passkey_new_login)) and hasRole(Role.RadioButton)).performClick()
        assertEquals(listOf("select:null"), calls)
    }

    @Test
    fun saveAndCancelAnswer() {
        show(state())
        // The answers are pinned under the scrolling list, so they are tapped without scrolling.
        rule.onNode(hasText(text(R.string.passkey_save)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.passkey_cancel)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertEquals(listOf("save", "cancel"), calls)
    }

    @Test
    fun closingTheSheetIsACancel() {
        show(state())
        rule.onNodeWithContentDescription(text(R.string.kit_close)).performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(listOf("cancel"), calls)
    }

    @Test
    fun closingTheSheetOverAnExistingPasskeyIsAClose() {
        show(state(excluded = true))
        rule.onNodeWithContentDescription(text(R.string.kit_close)).performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(listOf("close"), calls)
    }

    @Test
    fun aLongListAtALargeFontStillReachesSave() {
        show(state(homes = 30), fontScale = 1.5f)
        rule.onNode(hasText(text(R.string.passkey_save)) and hasClickAction()).assertIsDisplayed().performClick()
        assertEquals(listOf("save"), calls)
    }

    /** Review (stage 4): on a small phone the answers scrolled below the fold; they now stay pinned under the list. */
    @Test
    fun saveAndCancelStayInViewWhileTheLoginsScroll() {
        show(state(homes = 30))
        rule.onNode(hasText(text(R.string.passkey_save)) and hasClickAction()).assertIsDisplayed()
        rule.onNode(hasText(text(R.string.passkey_cancel)) and hasClickAction()).assertIsDisplayed()
        rule.onNode(hasText("Login 30") and hasRole(Role.RadioButton)).assertIsNotDisplayed()
    }

    @Test
    fun whileBusyClosingDoesNothingAndTheSheetStays() {
        show(state().copy(busy = true))
        rule.onAllNodesWithContentDescription(text(R.string.kit_close)).assertCountEquals(0)
        rule.onNodeWithTag(SHEET_TAG).assertIsDisplayed()
        assertEquals(emptyList<String>(), calls)
    }
}
