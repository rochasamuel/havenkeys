package net.havenkeys.android.ui.kit

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.activity.ComponentActivity
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.semantics.Role
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ScaffoldTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    @Test
    fun theSlotsStackTopContentBottom() {
        rule.setKit {
            HavenScaffold(topBar = { HavenText("Top") }, bottomBar = { HavenText("Bottom") }) { HavenText("Content") }
        }
        val top = rule.onNodeWithText("Top").getUnclippedBoundsInRoot()
        val content = rule.onNodeWithText("Content").getUnclippedBoundsInRoot()
        val bottom = rule.onNodeWithText("Bottom").getUnclippedBoundsInRoot()
        assertTrue(top.bottom <= content.top)
        assertTrue(content.bottom <= bottom.top)
    }

    @Test
    fun aFloatingButtonReservesRoomUnderTheContent() {
        rule.setKit {
            HavenScaffold(floatingButton = { AddButton(onClick = {}) }) { padding ->
                HavenText("pad ${padding.calculateBottomPadding()}")
            }
        }
        rule.onNodeWithText("pad 88.0.dp").assertExists()
        rule.onNodeWithContentDescription("New item").assertIsDisplayed()
    }

    @Test
    fun withoutOneTheContentKeepsAllItsRoom() {
        rule.setKit { HavenScaffold { padding -> HavenText("pad ${padding.calculateBottomPadding()}") } }
        rule.onNodeWithText("pad 0.0.dp").assertExists()
    }

    @Test
    fun aToastIsAnnouncedPolitelyThenLeaves() {
        rule.mainClock.autoAdvance = false
        rule.setKit {
            val toasts = rememberToastState()
            LaunchedEffect(Unit) { toasts.show("Password copied") }
            HavenScaffold(toastState = toasts) { HavenText("Content") }
        }
        rule.mainClock.advanceTimeBy(500)
        rule.onNodeWithText("Password copied")
            .assertIsDisplayed()
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite))
        rule.mainClock.advanceTimeBy(TOAST_MILLIS + 1_000)
        rule.onNodeWithText("Password copied").assertDoesNotExist()
    }

    @Test
    fun aToastsActionIsItsOwnButtonThatActsAndCloses() {
        rule.mainClock.autoAdvance = false
        var undone = 0
        rule.setKit {
            val toasts = rememberToastState()
            LaunchedEffect(Unit) { toasts.show("Moved “GitHub” to Trash.", action = ToastAction("Undo") { undone++ }) }
            HavenScaffold(toastState = toasts) { HavenText("Content") }
        }
        rule.mainClock.advanceTimeBy(500)
        // The message is announced without the button's label; the button is a 48dp target of its own.
        rule.onNodeWithText("Moved “GitHub” to Trash.")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Polite))
        rule.onNode(hasText("Undo") and hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.mainClock.advanceTimeBy(500)
        org.junit.Assert.assertEquals(1, undone)
        rule.onNodeWithText("Moved “GitHub” to Trash.").assertDoesNotExist()
    }

    @Test
    fun aToastWithAnActionStaysLongEnoughToReachIt() {
        rule.mainClock.autoAdvance = false
        rule.setKit {
            val toasts = rememberToastState()
            LaunchedEffect(Unit) { toasts.show("Moved “GitHub” to Trash.", action = ToastAction("Undo") {}) }
            HavenScaffold(toastState = toasts) { HavenText("Content") }
        }
        rule.mainClock.advanceTimeBy(TOAST_MILLIS + 1_000)
        rule.onNodeWithText("Undo").assertIsDisplayed()
        rule.mainClock.advanceTimeBy(TOAST_ACTION_MILLIS)
        rule.onNodeWithText("Undo").assertDoesNotExist()
    }

    @Test
    fun anAlertToastIsAnnouncedAssertively() {
        rule.mainClock.autoAdvance = false
        rule.setKit {
            val toasts = rememberToastState()
            LaunchedEffect(Unit) { toasts.show("HavenKeys is offline", ToastTone.Alert) }
            HavenScaffold(toastState = toasts) { HavenText("Content") }
        }
        rule.mainClock.advanceTimeBy(500)
        rule.onNodeWithText("HavenKeys is offline")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.LiveRegion, LiveRegionMode.Assertive))
    }

    @Test
    fun aToastStaysAsLongAsTheSystemRecommends() {
        rule.mainClock.autoAdvance = false
        val manager = object : androidx.compose.ui.platform.AccessibilityManager {
            var asked: Triple<Long, Boolean, Boolean>? = null

            override fun calculateRecommendedTimeoutMillis(
                originalTimeoutMillis: Long,
                containsIcons: Boolean,
                containsText: Boolean,
                containsControls: Boolean,
            ): Long {
                asked = Triple(originalTimeoutMillis, containsIcons, containsText)
                return 10_000L
            }
        }
        rule.setContent {
            androidx.compose.runtime.CompositionLocalProvider(
                androidx.compose.ui.platform.LocalAccessibilityManager provides manager,
            ) {
                net.havenkeys.android.ui.theme.HavenTheme {
                    val toasts = rememberToastState()
                    LaunchedEffect(Unit) { toasts.show("Password copied") }
                    HavenScaffold(toastState = toasts) { HavenText("Content") }
                }
            }
        }
        rule.mainClock.advanceTimeBy(TOAST_MILLIS + 1_000)
        rule.onNodeWithText("Password copied").assertIsDisplayed()
        org.junit.Assert.assertEquals(Triple(TOAST_MILLIS, true, true), manager.asked)
        rule.mainClock.advanceTimeBy(10_000)
        rule.onNodeWithText("Password copied").assertDoesNotExist()
    }

    @Test
    fun contentStaysClearOfSideInsets() {
        rule.runOnUiThread {
            androidx.core.view.WindowCompat.setDecorFitsSystemWindows(rule.activity.window, false)
        }
        rule.setKit { HavenScaffold { HavenText("Content") } }
        val before = rule.onNodeWithText("Content").getUnclippedBoundsInRoot()
        rule.runOnUiThread {
            val view = rule.activity.window.decorView
            val insets = androidx.core.view.WindowInsetsCompat.Builder()
                .setInsets(
                    androidx.core.view.WindowInsetsCompat.Type.systemBars(),
                    androidx.core.graphics.Insets.of(40, 0, 60, 0),
                )
                .build()
            androidx.core.view.ViewCompat.dispatchApplyWindowInsets(view, insets)
        }
        rule.waitForIdle()
        val after = rule.onNodeWithText("Content").getUnclippedBoundsInRoot()
        assertTrue("left ${before.left} -> ${after.left}", after.left > before.left)
    }
}
