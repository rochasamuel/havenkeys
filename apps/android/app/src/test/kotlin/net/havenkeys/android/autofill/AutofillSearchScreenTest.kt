package net.havenkeys.android.autofill

import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.AutofillMatch

@RunWith(RobolectricTestRunner::class)
class AutofillSearchScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private val repo = FakeAutofillRepository().apply {
        matchList = Outcome.Ok(listOf(AutofillMatch("m1", "GitHub", "sam", false)))
    }
    private val confirmed = mutableListOf<String>()

    private fun show(answer: String?) = rule.setKit {
        AutofillSearchScreen(
            search = repo::search,
            app = CallerApp("com.example.app", "Example"),
            onConfirmed = { match ->
                confirmed += match.id
                answer
            },
        )
    }

    private fun search(query: String) {
        rule.onNode(hasSetTextAction()).performTextInput(query)
        rule.waitForIdle()
        rule.mainClock.advanceTimeBy(500)
        rule.waitForIdle()
    }

    @Test
    fun aPickedLoginIsUsedOnlyAfterTheAppIsNamedAndConfirmed() {
        show(answer = null)
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNodeWithText(text(R.string.autofill_use_in_app, "GitHub", "com.example.app")).assertExists()
        rule.onNodeWithText(text(R.string.autofill_app_label, "Example")).assertExists()
        assertEquals(emptyList<String>(), confirmed)
        rule.onNode(hasText(text(R.string.autofill_use)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertEquals(listOf("m1"), confirmed)
    }

    @Test
    fun aRefusedFillSaysWhy() {
        show(answer = "not_found")
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.autofill_use)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        rule.onNodeWithText(text(errorText("not_found"))).assertExists()
    }

    @Test
    fun cancelUsesNothing() {
        show(answer = null)
        search("git")
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.onNode(hasText(text(R.string.autofill_cancel)) and hasAnyAncestor(isDialog())).performClick()
        rule.waitForIdle()
        assertEquals(emptyList<String>(), confirmed)
    }

    @Test
    fun nothingFoundSaysSo() {
        repo.matchList = Outcome.Ok(emptyList())
        show(answer = null)
        search("zzz")
        rule.onNodeWithText(text(R.string.vault_no_matches)).assertExists()
    }

    /** Final review (stage 4): "No matches" flashed while the query was still being waited on. */
    @Test
    fun noMatchesWaitsForTheAnswer() {
        repo.matchList = Outcome.Ok(emptyList())
        show(answer = null)
        rule.mainClock.autoAdvance = false
        rule.onNode(hasSetTextAction()).performTextInput("zzz")
        rule.mainClock.advanceTimeBy(100)
        rule.onAllNodesWithText(text(R.string.vault_no_matches)).assertCountEquals(0)
        rule.mainClock.advanceTimeBy(500)
        rule.waitForIdle()
        rule.onNodeWithText(text(R.string.vault_no_matches)).assertExists()
    }

    /** Final review (stage 4): a failed search looked like an empty result; it now says why. */
    @Test
    fun aFailedSearchSaysWhyAndNotNoMatches() {
        repo.matchList = Outcome.Failed("locked")
        show(answer = null)
        search("git")
        rule.onNodeWithText(text(errorText("locked"))).assertExists()
        rule.onAllNodesWithText(text(R.string.vault_no_matches)).assertCountEquals(0)
    }
}
