@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp

/**
 * The phone's type scale (spec 2026-10-03 §5.1): the desktop's faces,
 * weights and hierarchy (apps/desktop/DESIGN.md, Typography), with the
 * reading size raised for a phone. Sizes are sp, so text follows the system
 * font size. Serif names things, Hanken does the work, mono is for strings
 * that must be read exactly.
 */
object HavenTypography {
    // The desktop asks Hanken for "ss01", "cv11"; the font has ss01 only.
    private const val SANS_FEATURES = "'ss01'"

    private fun serif(weight: Int, size: Int, line: Int, tracking: Double = 0.0) = TextStyle(
        fontFamily = HavenSerif,
        fontWeight = FontWeight(weight),
        fontSize = size.sp,
        lineHeight = line.sp,
        letterSpacing = tracking.em,
    )

    private fun sans(weight: Int, size: Int, line: Int) = TextStyle(
        fontFamily = HankenGrotesk,
        fontWeight = FontWeight(weight),
        fontSize = size.sp,
        lineHeight = line.sp,
        fontFeatureSettings = SANS_FEATURES,
    )

    /** The unlock headline (its one brass italic is a SpanStyle there). */
    val display = serif(weight = 460, size = 32, line = 36, tracking = -0.015)

    /** Item and editor titles, a list screen's large title. */
    val headline = serif(weight = 480, size = 28, line = 32, tracking = -0.015)

    /** Sheet and dialog titles. */
    val title = serif(weight = 480, size = 22, line = 28, tracking = -0.012)

    /** Empty-state lines, small notices. */
    val titleSmall = serif(weight = 500, size = 18, line = 24)

    /** The initial in a monogram tile (ItemTile scales it to the tile). */
    val monogram = serif(weight = 520, size = 18, line = 22)

    /** The reading size. */
    val body = sans(weight = 400, size = 15, line = 22)

    /** A row's value: what the user opened the item to read. */
    val value = sans(weight = 400, size = 16, line = 22)

    /** The first line of an item row. */
    val rowTitle = sans(weight = 600, size = 16, line = 21)

    /** The second line of a row. */
    val rowSubtitle = sans(weight = 400, size = 14, line = 19)

    /** The label above a value. */
    val label = sans(weight = 560, size = 13, line = 17)

    /** A group's title, sentence case. */
    val groupTitle = sans(weight = 600, size = 14, line = 19)

    /** Button labels. */
    val button = sans(weight = 550, size = 16, line = 20)

    /** Pills and markers. */
    val pill = sans(weight = 600, size = 12, line = 16)

    /** A revealed password, key or setup string. */
    val secret = TextStyle(
        fontFamily = JetBrainsMono,
        fontWeight = FontWeight(500),
        fontSize = 16.sp,
        lineHeight = 22.sp,
        letterSpacing = 0.02.em,
    )

    /** The fixed row of dots that stands for any hidden value. */
    val masked = sans(weight = 500, size = 16, line = 22).copy(letterSpacing = 0.18.em)

    /** A one-time code: tabular, so it does not jitter as it changes. */
    val code = TextStyle(
        fontFamily = JetBrainsMono,
        fontWeight = FontWeight(500),
        fontSize = 22.sp,
        lineHeight = 28.sp,
        letterSpacing = 0.05.em,
        fontFeatureSettings = "'tnum'",
    )
}
