package net.havenkeys.android.ui.kit

import android.view.KeyEvent
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.getValue
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
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
    fun twoFailuresWithTheSameErrorCanBothBeRetriedWhenTheKeyIsACounter() {
        var attempts = 0
        rule.setKit {
            var failures by remember { mutableStateOf(0) }
            HavenDialog(
                title = "Name?",
                onDismiss = {},
                confirm = DialogAction("Save", { attempts++ }),
                answerKey = failures,
                content = {
                    // Every failure shows the same message; only the counter changes.
                    HavenText("Wrong")
                    HavenButton("Fail", onClick = { failures++ })
                },
            )
        }
        repeat(2) { round ->
            rule.onNodeWithText("Save").performClick()
            rule.runOnIdle { assertEquals(round + 1, attempts) }
            rule.onNodeWithText("Save").assertIsNotEnabled()
            rule.onNodeWithText("Fail").performClick()
        }
        rule.onNodeWithText("Save").performClick()
        rule.runOnIdle { assertEquals(3, attempts) }
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

    @Test
    fun aTallSheetStopsShortOfTheTopAndScrollsToItsLastRow() {
        rule.setKit {
            HavenSheet(onDismiss = { dismissed++ }, title = "Save to") {
                repeat(TALL_ROWS) { HavenText("Login $it", Modifier.heightIn(min = 48.dp)) }
            }
        }
        rule.waitForIdle()
        val bounds = rule.onNodeWithTag(SHEET_TAG).fetchSemanticsNode().boundsInRoot
        assertTrue(bounds.top > 0f)
        // At most 90% of the window: the root is as tall as the window.
        val window = windowHeight()
        assertTrue("sheet ${bounds.height} of $window", bounds.height <= window * 0.9f + 1f)
        rule.onNodeWithTag(SHEET_TAG).performTouchInput { swipeUp(startY = centerY + 100f, endY = centerY - 100f) }
        rule.waitForIdle()
        assertEquals(0, dismissed)
        rule.onNodeWithText("Login ${TALL_ROWS - 1}").performScrollTo().assertIsDisplayed()
    }

    @Test
    fun aTallSheetsFooterStaysInViewUnderItsScrollingContent() {
        rule.setKit {
            HavenSheet(onDismiss = {}, title = "Save to", footer = { HavenText("Answer") }) {
                repeat(TALL_ROWS) { HavenText("Login $it", Modifier.heightIn(min = 48.dp)) }
            }
        }
        rule.waitForIdle()
        rule.onNodeWithText("Answer").assertIsDisplayed()
        rule.onNodeWithText("Login ${TALL_ROWS - 1}").assertIsNotDisplayed()
    }

    @Test
    fun aShortSheetStillClosesWhenDraggedFromItsContent() {
        showSheet()
        rule.onNodeWithText("Login").performTouchInput { swipeDown(startY = centerY, endY = centerY + 2_000f) }
        rule.waitForIdle()
        assertEquals(1, dismissed)
    }

    @Test
    fun aThirdAnswerStacksTheButtonsAndAnswersOnce() {
        val answers = mutableListOf<String>()
        rule.setKit {
            HavenDialog(
                title = "Fill your identity?",
                onDismiss = { answers += "dismiss" },
                confirm = DialogAction("Fill with documents", { answers += "confirm" }),
                dismiss = DialogAction("Cancel", { answers += "cancel" }),
                alternative = DialogAction("Fill without documents", { answers += "alternative" }),
            )
        }
        rule.onNodeWithText("Fill without documents").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.onNodeWithText("Fill with documents").performClick()
        pressBack()
        rule.waitForIdle()
        assertEquals(listOf("alternative"), answers)
    }

    @Test
    fun anUndismissibleDialogIgnoresBackAndStillAnswers() {
        var reloads = 0
        var backs = 0
        rule.setKit {
            HavenDialog(
                title = "Changed on another device",
                onDismiss = { backs++ },
                confirm = DialogAction("Reload", { reloads++ }),
                dismissible = false,
            )
        }
        pressBack()
        rule.onNodeWithText("Reload").assertIsEnabled().performClick()
        assertEquals(0, backs)
        assertEquals(1, reloads)
    }

    /** Final review (stage 4): an undismissible sheet (work in flight) ignores Back, drag and the backdrop. */
    @Test
    fun anUndismissibleSheetIgnoresBackAndDrag() {
        rule.setKit {
            HavenSheet(onDismiss = { dismissed++ }, title = "Saving", dismissible = false) { HavenText("Login") }
        }
        pressBack()
        rule.onNodeWithTag(SHEET_TAG).performTouchInput { swipeDown(startY = top + 1f, endY = bottom + height) }
        rule.waitForIdle()
        rule.onNodeWithContentDescription("Close").assertDoesNotExist()
        rule.onNodeWithTag(SHEET_TAG).assertIsDisplayed()
        rule.onNodeWithText("Login").assertIsDisplayed()
        assertEquals(0, dismissed)
    }

    @Test
    fun theAlternativeAnswerIsGatedByBusyNotByConfirmEnabled() {
        var busy by mutableStateOf(false)
        rule.setKit {
            HavenDialog(
                title = "Fill your identity?",
                onDismiss = {},
                confirm = DialogAction("Fill with documents", {}),
                alternative = DialogAction("Fill without documents", {}),
                confirmEnabled = false,
                busy = busy,
            )
        }
        rule.onNodeWithText("Fill with documents").assertIsNotEnabled()
        rule.onNodeWithText("Fill without documents").assertIsEnabled()
        busy = true
        rule.waitForIdle()
        rule.onNodeWithText("Fill without documents").assertIsNotEnabled()
    }

    private fun dispatchSheetInsets(ime: Int, nav: Int) = rule.runOnUiThread {
        val view = ShadowDialog.getLatestDialog().window!!.decorView
        val insets = WindowInsetsCompat.Builder()
            .setInsets(WindowInsetsCompat.Type.navigationBars(), Insets.of(0, 0, 0, nav))
            .setInsets(WindowInsetsCompat.Type.ime(), Insets.of(0, 0, 0, ime))
            .setVisible(WindowInsetsCompat.Type.ime(), ime > 0)
            .build()
        ViewCompat.dispatchApplyWindowInsets(view, insets)
    }

    private fun tallSheet() = rule.setKit {
        HavenSheet(onDismiss = {}, title = "Save to") {
            repeat(TALL_ROWS) { HavenText("Login $it", Modifier.heightIn(min = 48.dp)) }
        }
    }

    /** The dialog's root is the tall one; the host activity's root is empty. */
    private fun windowHeight() = rule.onAllNodes(isRoot()).fetchSemanticsNodes().maxOf { it.boundsInRoot.height }

    private fun sheetBottomGap(): Float {
        val window = windowHeight()
        return window - rule.onNodeWithTag(SHEET_TAG).fetchSemanticsNode().boundsInRoot.bottom
    }

    @Test
    fun theSheetRisesAboveTheKeyboardWithoutCountingTheNavBarTwice() {
        showSheet()
        rule.waitForIdle()
        dispatchSheetInsets(ime = 300, nav = 48)
        rule.waitForIdle()
        val gap = sheetBottomGap()
        assertTrue("gap $gap", gap in 295f..305f)
    }

    @Test
    fun withOnlyTheNavBarTheSheetSitsAboveIt() {
        showSheet()
        rule.waitForIdle()
        dispatchSheetInsets(ime = 0, nav = 48)
        rule.waitForIdle()
        // The surface pads its content by the bar; its bottom edge is the window's.
        val window = windowHeight()
        val text = rule.onNodeWithText("Login").fetchSemanticsNode().boundsInRoot.bottom
        assertTrue("text ${text} window $window", window - text >= 48f)
    }

    @Test
    fun theCapShrinksWithTheKeyboard() {
        tallSheet()
        rule.waitForIdle()
        val tall = rule.onNodeWithTag(SHEET_TAG).fetchSemanticsNode().boundsInRoot.height
        dispatchSheetInsets(ime = 300, nav = 48)
        rule.waitForIdle()
        val shrunk = rule.onNodeWithTag(SHEET_TAG).fetchSemanticsNode().boundsInRoot.height
        assertTrue("$tall -> $shrunk", shrunk < tall - 200f)
    }

    private companion object {
        const val TALL_ROWS = 60
    }
}
