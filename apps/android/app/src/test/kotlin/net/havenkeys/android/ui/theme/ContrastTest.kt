package net.havenkeys.android.ui.theme

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.graphics.luminance
import kotlin.math.max
import kotlin.math.min
import net.havenkeys.android.ui.shell.searchPill
import org.junit.Assert.assertTrue
import org.junit.Test

/** WCAG AA (4.5:1) for every text colour on every surface it is drawn on. */
class ContrastTest {
    private val themes = listOf("dark" to DarkHavenColors, "light" to LightHavenColors)

    private fun assertReads(what: String, ink: Color, ground: Color) {
        val a = ink.compositeOver(ground).luminance()
        val b = ground.luminance()
        val ratio = (max(a, b) + 0.05f) / (min(a, b) + 0.05f)
        assertTrue("$what: ${"%.2f".format(ratio)}:1, needs 4.5:1", ratio >= 4.5f)
    }

    @Test
    fun textReadsOnEverySurface() {
        for ((theme, c) in themes) {
            val surfaces = mapOf("pane" to c.pane, "list" to c.list, "group" to c.group, "raised" to c.raised)
            val inks = mapOf(
                "text" to c.text,
                "textStrong" to c.textStrong,
                "muted" to c.muted,
                "brassInk" to c.brassInk,
                "danger" to c.danger,
            )
            for ((surface, ground) in surfaces) {
                for ((name, ink) in inks) assertReads("$theme $name on $surface", ink, ground)
            }
        }
    }

    /**
     * The search pill and the segmented track share one ground (hover in light,
     * field in dark); their placeholder and unselected labels are muted.
     */
    @Test
    fun textReadsOnTheSearchPillAndTheSegmentedTrack() {
        for ((theme, c) in themes) {
            for ((name, ink) in mapOf("text" to c.text, "textStrong" to c.textStrong, "muted" to c.muted)) {
                assertReads("$theme $name on the search pill and segmented track", ink, c.searchPill)
            }
        }
    }

    @Test
    fun textReadsOnFilledControls() {
        for ((theme, c) in themes) {
            assertReads("$theme onPrimary", c.onPrimary, c.primary)
            assertReads("$theme onBrass", c.onBrass, c.brass)
            assertReads("$theme onDanger", c.onDanger, c.danger)
            assertReads("$theme toast", c.onGlass, c.glass.compositeOver(c.pane))
            assertReads("$theme pill", c.brassInk, c.brassSoft.compositeOver(c.group))
            assertReads("$theme monogram", c.avatarFg, c.avatarBg)
        }
    }
}
