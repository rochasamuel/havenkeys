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
        // Screens not yet rebuilt from ui/kit still read MaterialTheme.
        MaterialBridge(darkTheme, content)
    }
}

object HavenTheme {
    val colors: HavenColors
        @Composable @ReadOnlyComposable
        get() = LocalHavenColors.current

    val motion: HavenMotion
        @Composable @ReadOnlyComposable
        get() = LocalHavenMotion.current
}
