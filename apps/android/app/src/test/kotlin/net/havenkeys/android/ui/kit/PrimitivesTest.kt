package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.FocusInteraction
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.LightHavenColors
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class PrimitivesTest {
    @get:Rule
    val rule = createComposeRule()

    private fun colorOf(text: String): Color {
        val layouts = mutableListOf<TextLayoutResult>()
        rule.onNodeWithText(text).performSemanticsAction(SemanticsActions.GetTextLayoutResult) { it(layouts) }
        return layouts.single().layoutInput.style.color
    }

    @Test
    fun textTakesTheContentColourUnlessItNamesOne() {
        rule.setKit {
            Column {
                ProvideContentColor(Color.Red) {
                    HavenText("inherits")
                    HavenText("own", color = Color.Blue)
                }
                HavenText("plain")
            }
        }
        assertEquals(Color.Red, colorOf("inherits"))
        assertEquals(Color.Blue, colorOf("own"))
        assertEquals(LightHavenColors.text, colorOf("plain"))
    }

    @Test
    fun pressFeedbackLetsEveryClickThrough() {
        var clicks = 0
        rule.setKit {
            Box(
                Modifier.size(48.dp).clickable(
                    interactionSource = null,
                    indication = HavenPress,
                    role = Role.Button,
                ) { clicks++ },
            )
        }
        val target = rule.onNode(hasClickAction())
        target.performClick()
        target.performTouchInput {
            down(center)
            up()
        }
        rule.waitForIdle()
        assertEquals(2, clicks)
    }

    @After
    fun clearTheFocusSeam() {
        FocusDrawSeam.observer = null
    }

    @Test
    fun aFocusedPressTargetDrawsTheBrassRing() {
        var seen = false
        FocusDrawSeam.observer = { seen = it }
        val source = MutableInteractionSource()
        val tag = "target"
        rule.setKit {
            Box(
                Modifier.size(48.dp).testTag(tag).clickable(
                    interactionSource = source,
                    indication = HavenPress,
                    role = Role.Button,
                ) {},
            )
        }
        rule.waitForIdle()
        assertEquals(false, seen)
        val focus = FocusInteraction.Focus()
        rule.runOnIdle { runBlocking { source.emit(focus) } }
        rule.waitForIdle()
        assertEquals(true, seen)
        rule.runOnIdle { runBlocking { source.emit(FocusInteraction.Unfocus(focus)) } }
        rule.waitForIdle()
        assertEquals(false, seen)
    }
}
