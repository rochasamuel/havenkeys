package net.havenkeys.android.ui.nav

import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class TitleTravelTest {
    @Test
    fun theLastTapNamesTheTravellingRow() {
        val travel = TitleTravel()
        travel.tap("a", "recent")
        assertTrue(travel.isTapped("a", "recent"))
        assertFalse(travel.isTapped("a", "frequent"))
        travel.tap("b", "frequent")
        assertFalse(travel.isTapped("a", "recent"))
    }

    @Test
    fun clearForgetsTheTapOnLock() {
        val travel = TitleTravel()
        travel.tap("a", "recent")
        travel.clear()
        assertFalse(travel.isTapped("a", "recent"))
    }

    @Test
    fun theTileAndTheTitleTravelUnderTheirOwnKeys() {
        assertNotEquals(titleKey("a"), tileKey("a"))
        assertNotEquals(tileKey("a"), tileKey("b"))
    }
}
