package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertRangeInfoEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class MenuAndFeedbackTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aMenuItemIsAButtonThatClosesTheMenuAndActs() {
        var edits = 0
        var closes = 0
        rule.setKit {
            Box {
                HavenIconButton(HavenIcon.More, "More", onClick = {})
                HavenMenu(
                    expanded = true,
                    onDismiss = { closes++ },
                    items = listOf(
                        MenuItem("Edit", { edits++ }, HavenIcon.Edit),
                        MenuItem("Delete", {}, HavenIcon.Trash, danger = true),
                    ),
                )
            }
        }
        rule.onNodeWithText("Delete").assert(hasRole(Role.Button)).assertTouchTarget()
        rule.onNodeWithText("Edit").assert(hasRole(Role.Button)).performClick()
        assertEquals(1, edits)
        assertEquals(1, closes)
    }

    @Test
    fun aClosedMenuShowsNothing() {
        rule.setKit { HavenMenu(expanded = false, onDismiss = {}, items = listOf(MenuItem("Edit", {}))) }
        rule.onNodeWithText("Edit").assertDoesNotExist()
    }

    @Test
    fun aRingReportsItsProgressOrThatItIsWorking() {
        rule.setKit {
            Box {
                ProgressRing(0.25f, contentDescription = "8 seconds left")
                ProgressRing(null, contentDescription = "Loading")
            }
        }
        rule.onNodeWithContentDescription("8 seconds left").assertRangeInfoEquals(ProgressBarRangeInfo(0.25f, 0f..1f))
        rule.onNodeWithContentDescription("Loading").assertRangeInfoEquals(ProgressBarRangeInfo.Indeterminate)
    }

    @Composable
    private fun Rows() {
        LazyColumn(Modifier.fillMaxSize().testTag("list")) {
            items(30) { HavenText("Row $it", Modifier.height(48.dp)) }
        }
    }

    @Test
    fun whileRefreshingTheRingSaysSo() {
        rule.setKit { PullToRefresh(refreshing = true, onRefresh = {}) { Rows() } }
        rule.onNodeWithContentDescription("Refreshing").assertRangeInfoEquals(ProgressBarRangeInfo.Indeterminate)
    }

    @Test
    fun pullingFarEnoughRefreshesOnce() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        rule.onNodeWithTag("list").performTouchInput { swipeDown(startY = top + 10f, endY = bottom - 10f) }
        rule.waitForIdle()
        assertEquals(1, refreshes)
    }

    @Test
    fun aShortPullDoesNot() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        rule.onNodeWithTag("list").performTouchInput { swipeDown(startY = top + 10f, endY = top + 10f + 40.dp.toPx()) }
        rule.waitForIdle()
        assertEquals(0, refreshes)
    }

    @Test
    fun talkBackCanRefreshWithoutPulling() {
        var refreshes = 0
        rule.setKit { PullToRefresh(refreshing = false, onRefresh = { refreshes++ }) { Rows() } }
        val node = rule.onNode(SemanticsMatcher.keyIsDefined(SemanticsActions.CustomActions)).fetchSemanticsNode()
        node.config[SemanticsActions.CustomActions].single { it.label == "Refresh" }.action()
        assertEquals(1, refreshes)
    }
}
