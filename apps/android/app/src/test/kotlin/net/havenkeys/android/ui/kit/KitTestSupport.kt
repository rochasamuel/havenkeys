package net.havenkeys.android.ui.kit

import android.provider.Settings
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertWidthIsAtLeast
import androidx.compose.ui.test.junit4.ComposeContentTestRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme
import org.robolectric.RuntimeEnvironment

/** Sets [content] inside [HavenTheme], optionally at a larger system font size. */
fun ComposeContentTestRule.setKit(dark: Boolean = false, fontScale: Float = 1f, content: @Composable () -> Unit) {
    setContent {
        val density = LocalDensity.current
        CompositionLocalProvider(LocalDensity provides Density(density.density, fontScale)) {
            HavenTheme(darkTheme = dark, content = content)
        }
    }
}

fun hasRole(role: Role): SemanticsMatcher = SemanticsMatcher.expectValue(SemanticsProperties.Role, role)

/** Spec §5.2: every touch target is at least 48dp each way. */
fun SemanticsNodeInteraction.assertTouchTarget(): SemanticsNodeInteraction =
    assertHeightIsAtLeast(48.dp).assertWidthIsAtLeast(48.dp)

/** The system's "Remove animations": the animator scale HavenTheme follows. Call before setKit. */
fun removeAnimations(on: Boolean = true) {
    Settings.Global.putFloat(
        RuntimeEnvironment.getApplication().contentResolver,
        Settings.Global.ANIMATOR_DURATION_SCALE,
        if (on) 0f else 1f,
    )
}
