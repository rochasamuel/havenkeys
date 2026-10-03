package net.havenkeys.android.ui.kit

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertRangeInfoEquals
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ChoicesTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aSwitchIsASwitchWithItsState() {
        var last = false
        rule.setKit {
            var checked by remember { mutableStateOf(false) }
            HavenSwitch(checked, onCheckedChange = { checked = it; last = it }, label = "Biometrics")
        }
        rule.onNodeWithContentDescription("Biometrics")
            .assert(hasRole(Role.Switch)).assertIsOff().assertTouchTarget()
            .performClick().assertIsOn()
        assertTrue(last)
    }

    @Test
    fun aToggleRowTogglesFromAnywhereOnTheRow() {
        rule.setKit {
            var checked by remember { mutableStateOf(true) }
            ToggleRow("Lock when the screen turns off", checked, { checked = it }, detail = "Recommended")
        }
        rule.onNodeWithText("Lock when the screen turns off")
            .assert(hasRole(Role.Switch)).assertIsOn().assertTouchTarget()
            .performClick().assertIsOff()
    }

    @Test
    fun aToggleRowsSwitchTrackEndsOnTheRowTextsMargin() {
        // The switch box is 52dp around a 44dp track: 4dp of air each side, so 12 + 4 = 16.
        assertEquals(net.havenkeys.android.ui.theme.HavenSpacing.rowX, TOGGLE_END + 4.dp)
    }

    @Test
    fun aSliderTakesTalkBacksSetProgressSnappedToItsSteps() {
        var last = 20f
        rule.setKit {
            var length by remember { mutableFloatStateOf(20f) }
            HavenSlider(
                length,
                { length = it; last = it },
                8f..64f,
                label = "Length",
                steps = 55,
                valueText = "${length.roundToInt()}",
            )
        }
        val slider = rule.onNodeWithContentDescription("Length")
        slider.assertRangeInfoEquals(ProgressBarRangeInfo(20f, 8f..64f, 55))
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "20"))
            .assertHeightIsAtLeast(48.dp)
        slider.performSemanticsAction(SemanticsActions.SetProgress) { it(30.4f) }
        assertEquals(30f, last, 0.001f)
        slider.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "30"))
    }

    @Test
    fun tappingNearTheEndPicksAValueNearTheEnd() {
        var last = 20f
        rule.setKit { HavenSlider(20f, { last = it }, 8f..64f, label = "Length", steps = 55) }
        rule.onNodeWithContentDescription("Length").performTouchInput { click(Offset(width - 2f, centerY)) }
        assertTrue("picked $last", last > 60f)
    }

    @Test
    fun stepsSnapToTheNearestStep() {
        assertEquals(30f, snapToStep(30.4f, 8f..64f, 55), 0f)
        assertEquals(64f, snapToStep(99f, 8f..64f, 55), 0f)
        assertEquals(0.37f, snapToStep(0.37f, 0f..1f, 0), 0f)
    }

    @Test
    fun segmentsAreTabsWithOneSelected() {
        var picked = -1
        rule.setKit {
            var index by remember { mutableIntStateOf(0) }
            SegmentedControl(listOf("Random", "Words"), index, { index = it; picked = it })
        }
        rule.onNodeWithText("Random").assert(hasRole(Role.Tab)).assertIsSelected().assertHeightIsAtLeast(48.dp)
        rule.onNodeWithText("Words").assertIsNotSelected().performClick().assertIsSelected()
        assertEquals(1, picked)
    }
}
