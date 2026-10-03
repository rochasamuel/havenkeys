package net.havenkeys.android.ui.theme

import androidx.compose.ui.test.junit4.createComposeRule
import org.junit.Assert.assertEquals
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
}
