package net.havenkeys.android.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HavenMotionTest {
    @Test
    fun removeAnimationsCutsEveryDuration() {
        val motion = havenMotion(0f)
        assertTrue(motion.reduced)
        assertEquals(listOf(0, 0, 0, 0), durations(motion))
    }

    @Test
    fun anyOtherScaleKeepsTheTokenDurations() {
        val motion = havenMotion(0.5f)
        assertFalse(motion.reduced)
        assertEquals(listOf(240, 250, 420, 1200), durations(motion))
    }

    private fun durations(m: HavenMotion) = listOf(m.snapMillis, m.tickMillis, m.sealMillis, m.breathMillis)
}
