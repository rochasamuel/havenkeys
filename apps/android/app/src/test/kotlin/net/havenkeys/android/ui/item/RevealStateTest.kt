package net.havenkeys.android.ui.item

import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
class RevealStateTest {
    @get:Rule
    val rule = createComposeRule()

    private val scope = TestScope(StandardTestDispatcher())

    @Test
    fun aShownValueIsGoneAfter30Seconds() {
        val state = RevealState(scope)
        state.show("hunter2")
        scope.advanceTimeBy(29_999)
        assertEquals("hunter2", state.value)
        scope.advanceTimeBy(2)
        assertNull(state.value)
    }

    @Test
    fun clearEmptiesItAtOnce() {
        val state = RevealState(scope)
        state.show("hunter2")
        state.clear()
        assertNull(state.value)
    }

    @Test
    fun showingAgainRestartsTheTimer() {
        val state = RevealState(scope)
        state.show("first")
        scope.advanceTimeBy(20_000)
        state.show("second")
        scope.advanceTimeBy(20_000)
        assertEquals("second", state.value)
        scope.advanceTimeBy(10_001)
        assertNull(state.value)
    }

    @Test
    fun aClearedValueStaysClearedWhenTheOldTimerWouldFire() {
        val state = RevealState(scope)
        state.show("hunter2")
        state.clear()
        scope.runCurrent()
        scope.advanceTimeBy(31_000)
        assertNull(state.value)
    }

    @Test
    fun theComposableStateIsClearedWhenItsRowLeavesComposition() {
        var present by mutableStateOf(true)
        var captured: RevealState? = null
        rule.setContent {
            if (present) {
                Box { captured = rememberRevealState() }
            }
        }
        rule.runOnIdle { captured!!.show("hunter2") }
        rule.runOnIdle { assertEquals("hunter2", captured!!.value) }
        present = false
        rule.waitForIdle()
        assertNull(captured!!.value)
    }
}
