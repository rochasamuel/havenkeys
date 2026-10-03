package net.havenkeys.android.autofill

import android.view.KeyEvent
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
class WalletConfirmDialogTest {
    @get:Rule
    val rule = createComposeRule()

    private val answers = mutableListOf<String>()

    private fun show(alternative: String? = "Fill without documents") = rule.setKit {
        WalletConfirmDialog(
            question = "Fill your identity in shop.example?",
            detail = "The app calls itself Shop",
            confirmLabel = "Fill with documents",
            alternativeLabel = alternative,
            onConfirm = { answers += "confirm" },
            onAlternative = { answers += "alternative" },
            onDismiss = { answers += "dismiss" },
        )
    }

    private fun pressBack() = rule.runOnIdle {
        val window = ShadowDialog.getLatestDialog().window!!
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
    }

    @Test
    fun theQuestionAndTheAppsOwnNameShow() {
        show()
        rule.onNodeWithText("Fill your identity in shop.example?").assertExists()
        rule.onNodeWithText("The app calls itself Shop").assertExists()
    }

    @Test
    fun eachAnswerAnswersOnce() {
        show()
        rule.onNodeWithText("Fill without documents").performClick()
        rule.onNodeWithText("Fill with documents").performClick()
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("alternative"), answers)
    }

    @Test
    fun backBeforeAnyAnswerCancels() {
        show(alternative = null)
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("dismiss"), answers)
    }
}
