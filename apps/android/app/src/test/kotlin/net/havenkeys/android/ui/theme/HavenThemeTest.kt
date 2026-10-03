package net.havenkeys.android.ui.theme

import androidx.compose.foundation.Indication
import androidx.compose.foundation.LocalIndication
import androidx.compose.ui.test.junit4.createComposeRule
import net.havenkeys.android.ui.kit.HavenPress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class HavenThemeTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun theFlagPicksTheColours() {
        var dark: HavenColors? = null
        var light: HavenColors? = null
        rule.setContent {
            HavenTheme(darkTheme = true) { dark = HavenTheme.colors }
            HavenTheme(darkTheme = false) { light = HavenTheme.colors }
        }
        rule.waitForIdle()
        assertEquals(DarkHavenColors, dark)
        assertEquals(LightHavenColors, light)
        assertEquals(true, dark?.isDark)
        assertEquals(false, light?.isDark)
    }

    @Test
    fun theDefaultIndicationIsTheKitPressNotARipple() {
        var indication: Indication? = null
        rule.setContent {
            HavenTheme(darkTheme = false) { indication = LocalIndication.current }
        }
        rule.waitForIdle()
        // A clickable that names no indication presses like the kit: no ripple, no grey highlight.
        assertSame(HavenPress, indication)
    }
}
