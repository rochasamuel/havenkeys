package net.havenkeys.android.ui.edit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.ui.test.junit4.createComposeRule
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class DraftTextTest {
    @get:Rule
    val rule = createComposeRule()

    private val edits = mutableListOf<String>()
    private lateinit var state: TextFieldState

    @Test
    fun theStartingValueIsNotAnEdit() {
        rule.setContent { state = rememberDraftText(initial = { "start" }, onEdit = { edits += it }) }
        rule.waitForIdle()
        assertEquals(emptyList<String>(), edits)
        rule.runOnIdle { state.setTextAndPlaceCursorAtEnd("next") }
        rule.waitForIdle()
        assertEquals(listOf("next"), edits)
    }

    /**
     * Final review (stage 4): with drop(1), an edit made before the collector started was the first
     * value seen and was dropped as if it were the start.
     */
    @Test
    fun anEditSeenFirstIsStillAnEdit() = runTest {
        assertEquals(listOf("early", "x"), editsAfter("start", flowOf("early", "x")).toList())
        assertEquals(listOf("x"), editsAfter("start", flowOf("start", "x")).toList())
    }

    @Test
    fun goingBackToTheStartingValueIsAnEdit() {
        rule.setContent { state = rememberDraftText(initial = { "start" }, onEdit = { edits += it }) }
        rule.runOnIdle { state.setTextAndPlaceCursorAtEnd("other") }
        rule.waitForIdle()
        rule.runOnIdle { state.setTextAndPlaceCursorAtEnd("start") }
        rule.waitForIdle()
        assertEquals(listOf("other", "start"), edits)
    }
}
