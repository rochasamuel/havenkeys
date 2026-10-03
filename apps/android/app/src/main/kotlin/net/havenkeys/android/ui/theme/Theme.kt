package net.havenkeys.android.ui.theme

import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import net.havenkeys.android.ui.kit.HavenPress

internal val LocalHavenColors = staticCompositionLocalOf { DarkHavenColors }
internal val LocalHavenMotion = staticCompositionLocalOf { havenMotion(1f) }

/**
 * Light and dark follow the system, like the desktop's "match system".
 * The kit's press is the default indication, so a clickable that names none
 * still presses the HavenKeys way: no ripple (spec 2026-10-03 §5).
 */
@Composable
fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalHavenColors provides if (darkTheme) DarkHavenColors else LightHavenColors,
        LocalHavenMotion provides rememberHavenMotion(),
        LocalIndication provides HavenPress,
        content = content,
    )
}

object HavenTheme {
    val colors: HavenColors
        @Composable @ReadOnlyComposable
        get() = LocalHavenColors.current

    val type: HavenTypography
        get() = HavenTypography

    val motion: HavenMotion
        @Composable @ReadOnlyComposable
        get() = LocalHavenMotion.current
}
