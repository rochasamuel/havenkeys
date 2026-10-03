package net.havenkeys.android.ui.theme

/*
 * Material, kept only so the material3 dependency still has a theme while it
 * is on the classpath. No screen reads it any more (stage 4); stage 5
 * deletes this file, the call in HavenTheme, and the material3 and
 * material-icons-extended dependencies. Nothing may use what is here.
 */

import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

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

private val Body = FontWeight(500) // --weight-body
private val Label = FontWeight(560) // --weight-label
private val Heading = FontWeight(640) // --weight-heading
private val Title = FontWeight(650) // --weight-title

private val TrackingTight = (-0.01).em // --tracking-tight
private val TrackingWide = 0.02.em // --tracking-wide

private val base = Typography()

internal val MaterialTypography = Typography(
    displayLarge = base.displayLarge.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    displayMedium = base.displayMedium.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    displaySmall = base.displaySmall.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    headlineLarge = base.headlineLarge.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    headlineMedium = base.headlineMedium.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    headlineSmall = base.headlineSmall.copy(fontWeight = Heading, letterSpacing = TrackingTight),
    titleLarge = base.titleLarge.copy(fontWeight = Title, letterSpacing = TrackingTight),
    titleMedium = base.titleMedium.copy(fontWeight = Title, letterSpacing = 0.em),
    titleSmall = base.titleSmall.copy(fontWeight = Title, letterSpacing = 0.em),
    bodyLarge = base.bodyLarge.copy(fontWeight = Body, letterSpacing = 0.em),
    bodyMedium = base.bodyMedium.copy(fontWeight = Body, letterSpacing = 0.em),
    bodySmall = base.bodySmall.copy(fontWeight = Body, letterSpacing = 0.em),
    labelLarge = base.labelLarge.copy(fontWeight = Label, letterSpacing = TrackingWide),
    labelMedium = base.labelMedium.copy(fontWeight = Label, letterSpacing = TrackingWide),
    labelSmall = base.labelSmall.copy(fontWeight = Label, letterSpacing = TrackingWide),
)

/** Styles for literal machine strings: revealed secrets, the mask, codes. */
object HavenType {
    /** A revealed password or key; --tracking-secret. */
    val secret = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontWeight = Body,
        fontSize = 16.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.06.em,
    )

    /** The fixed row of dots that stands for any hidden value; --tracking-masked. */
    val masked = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = Body,
        fontSize = 16.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.18.em,
    )

    /** A one-time code; --tracking-code, tabular figures so it does not jitter. */
    val code = TextStyle(
        fontFamily = FontFamily.Monospace,
        fontWeight = Heading,
        fontSize = 28.sp,
        lineHeight = 36.sp,
        letterSpacing = 0.05.em,
        fontFeatureSettings = "tnum",
    )
}

/** --r-sm, --r-md, --r-lg, then the site's panel and stage radii (apps/web/DESIGN.md). */
internal val MaterialShapes = Shapes(
    extraSmall = RoundedCornerShape(5.dp),
    small = RoundedCornerShape(8.dp),
    medium = RoundedCornerShape(10.dp),
    large = RoundedCornerShape(14.dp),
    extraLarge = RoundedCornerShape(18.dp),
)

/** The Material layer under [HavenTheme], until stage 5. */
@Composable
internal fun MaterialBridge(darkTheme: Boolean, content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkScheme else LightScheme,
        typography = MaterialTypography,
        shapes = MaterialShapes,
        content = content,
    )
}
