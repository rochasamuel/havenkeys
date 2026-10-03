package net.havenkeys.android.ui.kit

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChoiceSheetTest {
    @get:Rule
    val rule = createComposeRule()

    private val picked = mutableListOf<Int>()
    private var closed = 0

    private fun show() = rule.setKit {
        ChoiceSheet(
            title = "Lock automatically",
            values = listOf(1, 5, 15),
            selected = 5,
            label = { "After $it minutes" },
            onSelect = { picked += it },
            onDismiss = { closed++ },
        )
    }

    @Test
    fun eachValueIsARadioButtonAndTheCurrentOneIsSelected() {
        show()
        rule.onNode(hasText("After 5 minutes") and hasRole(Role.RadioButton)).assertIsSelected().assertTouchTarget()
        rule.onNode(hasText("After 1 minutes") and hasRole(Role.RadioButton)).assertIsNotSelected()
    }

    @Test
    fun pickingAnotherValueClosesThenSaves() {
        show()
        rule.onNodeWithText("After 15 minutes").performClick()
        assertEquals(listOf(15), picked)
        assertEquals(1, closed)
    }

    @Test
    fun pickingTheCurrentValueOnlyCloses() {
        show()
        rule.onNodeWithText("After 5 minutes").performClick()
        assertEquals(emptyList<Int>(), picked)
        assertEquals(1, closed)
    }
}
