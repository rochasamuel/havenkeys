package net.havenkeys.android.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/*
 * Every value comes from packages/ui/src/tokens.css, the desktop's tokens.
 *
 * Material 3 role          dark (:root)            light ([data-theme=light])
 * background, surface      --bg          #0f1614   --bg           #ffffff
 * surfaceContainerLowest   --bg-side     #0b110f   --bg           #ffffff
 * surfaceContainerLow      --bg-list     #121b18   --bg-list      #f4f7f5
 * surfaceContainer         --raised      #172320   --bg-list      #f4f7f5
 * surfaceContainerHigh     --hover       #1b2925   --hover        #e9efec
 * surfaceContainerHighest  --selected    #1f2f2a   --line         #e2e8e5
 * surfaceVariant           --raised      #172320   --bg-list      #f4f7f5
 * surfaceBright            --selected    #1f2f2a   --bg           #ffffff
 * surfaceDim, surfaceTint  --bg          #0f1614   --hover/--bg   #e9efec/#ffffff
 * onSurface, onBackground  --text        #e2eae6   --text         #1e2b27
 * onSurfaceVariant         --muted       #86968f   --muted        #66756f
 * outline                  --line-strong #2c3d37   --line-strong  #cbd5d0
 * outlineVariant           --line        #1f2c28   --line         #e2e8e5
 * primary                  --primary-bg  #c9a45c   --primary-bg   #16231f
 * onPrimary                --primary-fg  #121a17   --primary-fg   #f1f5f3
 * primaryContainer,        --brass-soft over --bg  --brass-soft over --bg
 *   secondaryContainer       #292a1e                 #f6f0e5
 * onPrimaryContainer,      --brass-ink   #e3c483   --text         #1e2b27
 *   onSecondaryContainer
 * secondary                --brass       #c9a45c   --brass-ink    #8f7236
 * onSecondary              --on-brass    #121a17   --primary-fg   #f1f5f3
 * tertiary                 --ok          #5fa785   --ok           #3d6b58
 * onTertiary               --on-brass    #121a17   --primary-fg   #f1f5f3
 * inversePrimary           --brass-ink(l)#8f7236   --brass        #c9a45c
 * error                    --danger      #e0775f   --danger       #b4412f
 * onError                  --on-danger   #ffffff   --on-danger    #ffffff
 * errorContainer           --danger 12% over --bg  --danger 12% over --bg
 *                            #28221d                 #f6e8e6
 * inverseSurface           --text-strong #f3f7f5   --bg-side      #16231f
 * inverseOnSurface         --on-brass    #121a17   --side-text    #c3cfc9
 *
 * Light text on the brass containers is --text, not --brass-ink: brass-ink
 * on brass-soft is 4.0:1, under the 4.5:1 small text needs.
 *
 * Material's opaque containers cannot take --brass-soft's alpha, so it is
 * composited over --bg once here; HavenColors.brassSoft keeps the alpha for
 * tints drawn over other surfaces. Dynamic colour is off on purpose: the
 * vault must look like HavenKeys on every phone.
 */

private val DarkBg = Color(0xFF0F1614)
private val LightBg = Color(0xFFFFFFFF)

/** The brand roles Material's ColorScheme has no slot for. */
@Immutable
data class HavenColors(
    val textStrong: Color,
    val brass: Color,
    val brassHi: Color,
    val brassInk: Color,
    val brassSoft: Color,
    val ok: Color,
    val danger: Color,
    val digit: Color,
    val symbol: Color,
    val avatarBg: Color,
    val avatarFg: Color,
    val onBrass: Color,
)

val DarkHavenColors = HavenColors(
    textStrong = Color(0xFFF3F7F5),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFFE3C483),
    brassSoft = Color(0x24C9A45C),
    ok = Color(0xFF5FA785),
    danger = Color(0xFFE0775F),
    digit = Color(0xFFE3C483),
    symbol = Color(0xFF8FC4AD),
    avatarBg = Color(0xFF1D2B27),
    avatarFg = Color(0xFFE3C483),
    onBrass = Color(0xFF121A17),
)

val LightHavenColors = HavenColors(
    textStrong = Color(0xFF121A17),
    brass = Color(0xFFC9A45C),
    brassHi = Color(0xFFE3C483),
    brassInk = Color(0xFF8F7236),
    brassSoft = Color(0x29C9A45C),
    ok = Color(0xFF3D6B58),
    danger = Color(0xFFB4412F),
    digit = Color(0xFF8F7236),
    symbol = Color(0xFF3D6B58),
    avatarBg = Color(0xFF16231F),
    avatarFg = Color(0xFFE3C483),
    onBrass = Color(0xFF121A17),
)

internal val DarkScheme: ColorScheme = darkColorScheme(
    primary = Color(0xFFC9A45C),
    onPrimary = Color(0xFF121A17),
    primaryContainer = Color(0xFF292A1E),
    onPrimaryContainer = Color(0xFFE3C483),
    inversePrimary = Color(0xFF8F7236),
    secondary = Color(0xFFC9A45C),
    onSecondary = Color(0xFF121A17),
    secondaryContainer = Color(0xFF292A1E),
    onSecondaryContainer = Color(0xFFE3C483),
    tertiary = Color(0xFF5FA785),
    onTertiary = Color(0xFF121A17),
    background = DarkBg,
    onBackground = Color(0xFFE2EAE6),
    surface = DarkBg,
    onSurface = Color(0xFFE2EAE6),
    surfaceVariant = Color(0xFF172320),
    onSurfaceVariant = Color(0xFF86968F),
    surfaceTint = DarkBg,
    inverseSurface = Color(0xFFF3F7F5),
    inverseOnSurface = Color(0xFF121A17),
    error = Color(0xFFE0775F),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFF28221D),
    onErrorContainer = Color(0xFFE0775F),
    outline = Color(0xFF2C3D37),
    outlineVariant = Color(0xFF1F2C28),
    scrim = Color(0xFF000000),
    surfaceBright = Color(0xFF1F2F2A),
    surfaceDim = DarkBg,
    surfaceContainerLowest = Color(0xFF0B110F),
    surfaceContainerLow = Color(0xFF121B18),
    surfaceContainer = Color(0xFF172320),
    surfaceContainerHigh = Color(0xFF1B2925),
    surfaceContainerHighest = Color(0xFF1F2F2A),
)

internal val LightScheme: ColorScheme = lightColorScheme(
    primary = Color(0xFF16231F),
    onPrimary = Color(0xFFF1F5F3),
    primaryContainer = Color(0xFFF6F0E5),
    onPrimaryContainer = Color(0xFF1E2B27),
    inversePrimary = Color(0xFFC9A45C),
    secondary = Color(0xFF8F7236),
    onSecondary = Color(0xFFF1F5F3),
    secondaryContainer = Color(0xFFF6F0E5),
    onSecondaryContainer = Color(0xFF1E2B27),
    tertiary = Color(0xFF3D6B58),
    onTertiary = Color(0xFFF1F5F3),
    background = LightBg,
    onBackground = Color(0xFF1E2B27),
    surface = LightBg,
    onSurface = Color(0xFF1E2B27),
    surfaceVariant = Color(0xFFF4F7F5),
    onSurfaceVariant = Color(0xFF66756F),
    surfaceTint = LightBg,
    inverseSurface = Color(0xFF16231F),
    inverseOnSurface = Color(0xFFC3CFC9),
    error = Color(0xFFB4412F),
    onError = Color(0xFFFFFFFF),
    errorContainer = Color(0xFFF6E8E6),
    onErrorContainer = Color(0xFFB4412F),
    outline = Color(0xFFCBD5D0),
    outlineVariant = Color(0xFFE2E8E5),
    scrim = Color(0xFF000000),
    surfaceBright = LightBg,
    surfaceDim = Color(0xFFE9EFEC),
    surfaceContainerLowest = LightBg,
    surfaceContainerLow = Color(0xFFF4F7F5),
    surfaceContainer = Color(0xFFF4F7F5),
    surfaceContainerHigh = Color(0xFFE9EFEC),
    surfaceContainerHighest = Color(0xFFE2E8E5),
)
