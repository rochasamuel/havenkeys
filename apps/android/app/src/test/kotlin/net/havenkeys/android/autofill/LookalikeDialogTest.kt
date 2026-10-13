package net.havenkeys.android.autofill

import android.app.Activity
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.runBlocking
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows
import uniffi.havenkeys_mobile.TargetFacts

private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

@RunWith(RobolectricTestRunner::class)
class LookalikeDialogTest {
    @get:Rule
    val rule = createComposeRule()

    private var dismissed = 0

    private fun show() = rule.setKit { LookalikeDialog(site = "github.com", onDismiss = { dismissed++ }) }

    @Test
    fun namesTheSavedSiteAndExplains() {
        show()
        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_title, "github.com")).assertExists()
        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_body, "github.com")).assertExists()
    }

    @Test
    fun theOnlyButtonClosesOnce() {
        show()
        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_close)).performClick()
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }
}

/** The activity's look-alike branch: the site comes from Rust, asked for the tapped page. */
@RunWith(RobolectricTestRunner::class)
class ExplainLookalikeTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val repo = FakeAutofillRepository()
    private val target = TargetFacts("com.android.chrome", listOf(ByteArray(32)), "xn--gthub-n2e.com", "https")

    private fun explain() = rule.runOnUiThread { runBlocking { rule.activity.explainLookalike(repo, target) } }

    private fun result() = Shadows.shadowOf(rule.activity).resultCode

    @Test
    fun rustNamesTheSiteForTheTappedPageAndClosingFillsNothing() {
        repo.lookalikeSite = Outcome.Ok("github.com")
        explain()
        assertSame("Rust is asked about the page Android attached", target, repo.lookalikeTargets.single())
        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_title, "github.com")).assertExists()
        assertFalse(rule.activity.isFinishing)

        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_close)).performClick()
        rule.waitForIdle()
        assertTrue(rule.activity.isFinishing)
        assertEquals(Activity.RESULT_CANCELED, result())
        assertEquals("nothing but the look-alike question reached Rust", listOf("lookalike"), repo.calls)
    }

    private fun assertCancelledWithoutScreen() {
        assertTrue(rule.activity.isFinishing)
        assertEquals(Activity.RESULT_CANCELED, result())
        rule.onNodeWithText(text(R.string.autofill_lookalike_dialog_close)).assertDoesNotExist()
    }

    @Test
    fun noLookalikeFromRustMeansNoScreen() {
        repo.lookalikeSite = Outcome.Ok(null)
        explain()
        assertCancelledWithoutScreen()
    }

    @Test
    fun aRustErrorMeansNoScreen() {
        repo.lookalikeSite = Outcome.Failed("locked")
        explain()
        assertCancelledWithoutScreen()
    }
}
