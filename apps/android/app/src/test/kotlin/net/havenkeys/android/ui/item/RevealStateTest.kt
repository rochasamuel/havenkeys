package net.havenkeys.android.ui.item

import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class RevealStateTest {
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
}
