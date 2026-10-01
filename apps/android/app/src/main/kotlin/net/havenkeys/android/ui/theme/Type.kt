package net.havenkeys.android.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/*
 * Weights and tracking from packages/ui/src/tokens.css; sizes are Material's
 * scale in sp, so text follows the system font size. The desktop's faces
 * (Hanken Grotesk, JetBrains Mono) are not bundled: the system sans and
 * monospace stand in, and nothing is downloaded at run time.
 */

private val Body = FontWeight(500) // --weight-body
private val Label = FontWeight(560) // --weight-label
private val Heading = FontWeight(640) // --weight-heading
private val Title = FontWeight(650) // --weight-title

private val TrackingTight = (-0.01).em // --tracking-tight
private val TrackingWide = 0.02.em // --tracking-wide

private val base = Typography()

val HavenTypography = Typography(
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
