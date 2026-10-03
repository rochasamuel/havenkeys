package net.havenkeys.android.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import net.havenkeys.android.ui.theme.havenMotion
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Test

class NavMotionTest {
    private val full = havenMotion(1f)
    private val reduced = havenMotion(0f)

    @Test
    fun theLockAndTheRemovalCut() {
        assertEquals(Move.CUT, outerMove(Routes.ITEM, Routes.UNLOCK))
        assertEquals(Move.CUT, outerMove(Routes.SHELL, Routes.UNLOCK))
        assertEquals(Move.CUT, outerMove(Routes.SHELL, Routes.ONBOARDING))
        assertEquals(EnterTransition.None, enterFor(Move.CUT, full, pop = false))
        assertEquals(ExitTransition.None, exitFor(Move.CUT, full, pop = false))
    }

    @Test
    fun unlockingRevealsTheShellOnTheSeal() {
        assertEquals(Move.SEAL, outerMove(Routes.UNLOCK, Routes.SHELL))
        assertEquals(Move.SEAL, outerMove(Routes.ONBOARDING, Routes.SHELL))
    }

    @Test
    fun screensOverTheShellArePushedAndPopped() {
        for (route in PUSHED) {
            assertEquals(route, Move.PUSH, outerMove(Routes.SHELL, route))
            assertEquals(route, Move.PUSH, outerMove(route, Routes.SHELL))
        }
        assertEquals(Move.PUSH, outerMove(Routes.SEARCH, Routes.ITEM))
        assertEquals(Move.PUSH, outerMove(Routes.ITEM, Routes.EDIT))
    }

    @Test
    fun searchFadesUnderThePill() {
        assertEquals(Move.FADE, outerMove(Routes.SHELL, Routes.SEARCH))
        assertEquals(Move.FADE, outerMove(Routes.SEARCH, Routes.SHELL))
    }

    @Test
    fun removeAnimationsCutsEveryMove() {
        for (move in Move.entries) {
            for (pop in listOf(false, true)) {
                assertEquals("$move", EnterTransition.None, enterFor(move, reduced, pop))
                assertEquals("$move", ExitTransition.None, exitFor(move, reduced, pop))
            }
        }
    }

    @Test
    fun withAnimationsEveryMoveButTheCutMoves() {
        for (move in Move.entries - Move.CUT) {
            for (pop in listOf(false, true)) {
                assertNotEquals("$move", EnterTransition.None, enterFor(move, full, pop, tabShiftPx = 12))
                assertNotEquals("$move", ExitTransition.None, exitFor(move, full, pop))
            }
        }
    }
}
