package net.havenkeys.android.ui.kit

import android.view.KeyEvent
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.shadows.ShadowDialog

@RunWith(RobolectricTestRunner::class)
class OverlaysTest {
    @get:Rule
    val rule = createComposeRule()

    private var dismissed = 0

    private fun showSheet() = rule.setKit {
        HavenSheet(onDismiss = { dismissed++ }, title = "New item") { HavenText("Login") }
    }

    @Test
    fun aSheetIsADialogWithItsTitleAndClosesFromTheBackdrop() {
        showSheet()
        rule.onNode(isDialog()).assertExists()
        rule.onNodeWithText("New item").assert(isHeading())
        rule.onNodeWithText("Login").assertIsDisplayed()
        rule.onNodeWithContentDescription("Close")
            .assert(hasRole(Role.Button))
            .performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun draggingTheSheetDownDismissesIt() {
        showSheet()
        rule.onNodeWithTag(SHEET_TAG).performTouchInput { swipeDown(startY = top + 1f, endY = bottom + height) }
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun underRemoveAnimationsTheSheetClosesAtOnce() {
        removeAnimations()
        rule.mainClock.autoAdvance = false
        showSheet()
        repeat(3) { rule.mainClock.advanceTimeByFrame() }
        rule.onNodeWithContentDescription("Close").performSemanticsAction(SemanticsActions.OnClick)
        repeat(3) { rule.mainClock.advanceTimeByFrame() }
        assertEquals(1, dismissed)
    }

    @Test
    fun aTapWhileTheSheetIsStillOpeningClosesItOnce() {
        rule.mainClock.autoAdvance = false
        showSheet()
        rule.mainClock.advanceTimeBy(48)
        rule.onNodeWithContentDescription("Close").performSemanticsAction(SemanticsActions.OnClick)
        rule.mainClock.autoAdvance = true
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun aDialogNamesItselfAndAnswersOnce() {
        var removed = 0
        rule.setKit {
            HavenDialog(
                title = "Remove this device?",
                onDismiss = { dismissed++ },
                confirm = DialogAction("Remove", { removed++ }, danger = true),
                message = "Its local copy of the vault is erased.",
                dismiss = DialogAction("Cancel", { dismissed++ }),
            )
        }
        rule.onNode(isDialog()).assertExists()
        rule.onNodeWithText("Remove this device?").assert(isHeading())
        val remove = rule.onNodeWithText("Remove")
        remove.assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        remove.performClick()
        assertEquals(1, removed)
        rule.onNodeWithText("Cancel").assertIsNotEnabled()
        assertEquals(0, dismissed)
    }

    private fun showDialog(onConfirm: () -> Unit) = rule.setKit {
        HavenDialog(title = "Remove?", onDismiss = { dismissed++ }, confirm = DialogAction("Remove", onConfirm))
    }

    private fun pressBack() = rule.runOnIdle {
        val window = ShadowDialog.getLatestDialog().window!!
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_BACK))
        window.callback.dispatchKeyEvent(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_BACK))
    }

    @Test
    fun backAfterAConfirmDoesNotAnswerTwice() {
        var removed = 0
        showDialog { removed++ }
        rule.onNodeWithText("Remove").performClick()
        pressBack()
        rule.waitForIdle()
        assertEquals(1, removed)
        assertEquals(0, dismissed)
    }

    @Test
    fun backBeforeAnyAnswerDismissesOnce() {
        showDialog {}
        pressBack()
        pressBack()
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun aDialogLeftOpenAfterAFailedConfirmAnswersAgain() {
        var attempts = 0
        rule.setKit {
            var busy by remember { mutableStateOf(false) }
            HavenDialog(
                title = "Name?",
                onDismiss = {},
                confirm = DialogAction("Save", { attempts++; busy = true }),
                busy = busy,
                content = {
                    HavenText("go")
                    HavenButton("Fail", onClick = { busy = false })
                },
            )
        }
        rule.onNodeWithText("Save").performClick()
        rule.runOnIdle { assertEquals(1, attempts) }
        rule.onNodeWithText("Save").assertIsNotEnabled()
        rule.onNodeWithText("Fail").performClick()
        rule.onNodeWithText("Save").performClick()
        rule.runOnIdle { assertEquals(2, attempts) }
    }

    @Test
    fun aChangedAnswerKeyReArmsTheDialogAndADoubleTapStillAnswersOnce() {
        var attempts = 0
        rule.setKit {
            var error by remember { mutableStateOf<String?>(null) }
            HavenDialog(
                title = "Name?",
                onDismiss = {},
                confirm = DialogAction("Save", { attempts++ }),
                answerKey = error,
                content = { HavenButton("Err", onClick = { error = "wrong" }) },
            )
        }
        rule.onNodeWithText("Save").performClick()
        rule.onNodeWithText("Save").performClick()
        rule.runOnIdle { assertEquals(1, attempts) }
        rule.onNodeWithText("Err").performClick()
        rule.onNodeWithText("Save").performClick()
        rule.runOnIdle { assertEquals(2, attempts) }
    }

    @Test
    fun whileBusyNothingDismissesTheDialog() {
        var removed = 0
        rule.setKit {
            HavenDialog(
                title = "Remove?",
                onDismiss = { dismissed++ },
                confirm = DialogAction("Remove", { removed++ }),
                dismiss = DialogAction("Cancel", { dismissed++ }),
                busy = true,
            )
        }
        rule.onNodeWithText("Cancel").assertIsNotEnabled()
        rule.onNodeWithText("Remove").assertIsNotEnabled()
        pressBack()
        rule.waitForIdle()
        assertEquals(0, dismissed)
        assertEquals(0, removed)
    }

    @Test
    fun theCallerCanDisableConfirm() {
        var removed = 0
        rule.setKit {
            HavenDialog(
                title = "Name?",
                onDismiss = {},
                confirm = DialogAction("Save", { removed++ }),
                confirmEnabled = false,
            )
        }
        rule.onNodeWithText("Save").assertIsNotEnabled().performClick()
        rule.runOnIdle { assertEquals(0, removed) }
    }

    @Test
    fun theContentSlotRendersAFieldBetweenMessageAndButtons() {
        val state = androidx.compose.foundation.text.input.TextFieldState()
        rule.setKit {
            HavenDialog(
                title = "Name?",
                onDismiss = {},
                confirm = DialogAction("Save", {}),
                message = "Pick a name",
                content = { HavenTextField(state, label = "Name", error = "Too short") },
            )
        }
        val field = rule.onNode(androidx.compose.ui.test.hasSetTextAction())
        field.assertIsDisplayed().performTextInput("abc")
        rule.runOnIdle { assertEquals("abc", state.text.toString()) }
        rule.onNodeWithText("Save").assertIsDisplayed()
    }
}
