package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.GraphicsMode

/**
 * Font-scale tests need real text measurement, so they run on Robolectric's
 * native graphics; the rest of the kit's tests stay on legacy graphics.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class KitLargeFontTest {
    @get:Rule
    val rule = createComposeRule()

    private fun SemanticsNodeInteraction.layout(): TextLayoutResult {
        val results = mutableListOf<TextLayoutResult>()
        fetchSemanticsNode().config.getOrNull(SemanticsActions.GetTextLayoutResult)?.action?.invoke(results)
        return results.first()
    }

    @Test
    fun theLargestFontWrapsTheButtonLabelInsteadOfCuttingIt() {
        val label = "Remove this device from your vault"
        rule.setKit(fontScale = 2f) { HavenButton(label, onClick = {}, modifier = Modifier.width(200.dp)) }
        val layout = rule.onNodeWithText(label, useUnmergedTree = true).layout()
        assertTrue("lines=${layout.lineCount}", layout.lineCount >= 2)
        assertFalse(layout.isLineEllipsized(layout.lineCount - 1))
        assertEquals(label.length, layout.getLineEnd(layout.lineCount - 1, visibleEnd = false))
        rule.onNodeWithText(label).assertHeightIsAtLeast(48.dp)
        val textHeight = layout.size.height
        rule.onNodeWithText(label).assertHeightIsAtLeast(with(rule.density) { textHeight.toDp() } + 24.dp)
    }

    @Test
    fun theLargestFontGrowsTheItemRowWithItsTwoLines() {
        rule.setKit(fontScale = 2f) {
            ItemRow("GitHub", "sam@example.com", RowLeading.Monogram("GitHub"), onClick = {})
        }
        val title = rule.onNodeWithText("GitHub", useUnmergedTree = true).layout()
        val sub = rule.onNodeWithText("sam@example.com", useUnmergedTree = true).layout()
        val lines = with(rule.density) { (title.size.height + sub.size.height).toDp() }
        assertTrue(lines > 64.dp - 20.dp)
        rule.onNodeWithText("GitHub").assertHeightIsAtLeast(lines + 20.dp)
    }

    @Test
    fun segmentsKeepOneHeightWhenALabelWraps() {
        rule.setKit(fontScale = 1.5f) {
            Box(Modifier.width(320.dp)) {
                SegmentedControl(listOf("Site inteiro, qualquer subdomínio", "Somente este site", "Página"), 0, {})
            }
        }
        val heights = rule.onAllNodes(hasRole(Role.Tab)).fetchSemanticsNodes().map { it.size.height }
        assertEquals(3, heights.size)
        assertEquals(1, heights.distinct().size)
    }
}
