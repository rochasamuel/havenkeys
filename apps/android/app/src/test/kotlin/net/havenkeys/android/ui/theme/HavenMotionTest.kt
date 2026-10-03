package net.havenkeys.android.ui.theme

import androidx.compose.animation.core.SnapSpec
import androidx.compose.animation.core.SpringSpec
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

    @Test
    fun springsCutUnderRemoveAnimations() {
        val motion = havenMotion(0f)
        listOf(HavenSprings.smooth, HavenSprings.sheet, HavenSprings.toast, HavenSprings.press).forEach {
            assertTrue(motion.springSpec<Float>(it) is SnapSpec<*>)
        }
        assertTrue(motion.fadeSpec<Float>() is SnapSpec<*>)
    }

    @Test
    fun springsUseApplesResponseAndDamping() {
        val motion = havenMotion(1f)
        val smooth = motion.springSpec<Float>(HavenSprings.smooth) as SpringSpec<Float>
        assertEquals(1f, smooth.dampingRatio, 0f)
        assertEquals(157.91f, smooth.stiffness, 0.01f)
        val sheet = motion.springSpec<Float>(HavenSprings.sheet) as SpringSpec<Float>
        assertEquals(0.86f, sheet.dampingRatio, 0f)
        assertEquals(246.74f, sheet.stiffness, 0.01f)
    }

    private fun durations(m: HavenMotion) = listOf(m.snapMillis, m.tickMillis, m.sealMillis, m.breathMillis)
}
