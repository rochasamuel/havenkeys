package net.havenkeys.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf

internal val LocalHavenColors = staticCompositionLocalOf { DarkHavenColors }
internal val LocalHavenMotion = staticCompositionLocalOf { havenMotion(1f) }

/** Light and dark follow the system, like the desktop's "match system". */
@Composable
fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalHavenColors provides if (darkTheme) DarkHavenColors else LightHavenColors,
        LocalHavenMotion provides rememberHavenMotion(),
    ) {
        // Nothing reads MaterialTheme since stage 4; stage 5 removes this wrapper with material3.
        MaterialBridge(darkTheme, content)
    }
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
