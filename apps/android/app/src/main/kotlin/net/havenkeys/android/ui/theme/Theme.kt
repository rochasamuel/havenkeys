package net.havenkeys.android.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.unit.dp

/** --r-sm, --r-md, --r-lg, then the site's panel and stage radii (apps/web/DESIGN.md). */
val HavenShapes = Shapes(
    extraSmall = RoundedCornerShape(5.dp),
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(10.dp),
    large = RoundedCornerShape(14.dp),
    extraLarge = RoundedCornerShape(18.dp),
)

private val LocalHavenColors = staticCompositionLocalOf { DarkHavenColors }
private val LocalHavenMotion = staticCompositionLocalOf { havenMotion(1f) }

/** Light and dark follow the system, like the desktop's "match system". */
@Composable
fun HavenTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    CompositionLocalProvider(
        LocalHavenColors provides if (darkTheme) DarkHavenColors else LightHavenColors,
        LocalHavenMotion provides rememberHavenMotion(),
    ) {
        MaterialTheme(
            colorScheme = if (darkTheme) DarkScheme else LightScheme,
            typography = HavenTypography,
            shapes = HavenShapes,
            content = content,
        )
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
