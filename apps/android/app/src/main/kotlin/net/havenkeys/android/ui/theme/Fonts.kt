package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.ExperimentalTextApi
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontVariation
import androidx.compose.ui.text.font.FontWeight
import net.havenkeys.android.R

/*
 * The desktop's three faces, bundled (res/font, built by
 * scripts/build-android-fonts.py; SIL OFL 1.1, see THIRD-PARTY-NOTICES.md).
 * Nothing is downloaded. The Hanken and serif files are variable: each
 * weight the desktop uses is declared once with its axis value, so the file
 * draws exactly that weight (variation settings apply from API 26; minSdk
 * is 28). Glyphs outside the Latin subset fall back to the system fonts.
 */

/** Hanken Grotesk weights in the desktop: body 400/500, controls 550, labels 560, emphasis 600, headings 640-660. */
internal val HankenWeights = listOf(400, 500, 550, 560, 600, 640, 650, 660)

/** Source Serif 4 roman weights in the desktop: 400 and the title weights 460-520. */
internal val SerifWeights = listOf(400, 460, 470, 480, 500, 520)

@OptIn(ExperimentalTextApi::class)
private fun variable(resId: Int, weight: Int): Font = Font(
    resId = resId,
    weight = FontWeight(weight),
    style = FontStyle.Normal,
    variationSettings = FontVariation.Settings(FontVariation.weight(weight)),
)

/** Everything that is not a title or a secret. */
val HankenGrotesk: FontFamily = FontFamily(HankenWeights.map { variable(R.font.hanken_grotesk, it) })

/** Titles and monograms (Source Serif 4, renamed for the OFL's Reserved Font Name). */
val HavenSerif: FontFamily = FontFamily(
    SerifWeights.map { variable(R.font.havenkeys_serif, it) } +
        Font(R.font.havenkeys_serif_italic, FontWeight(400), FontStyle.Italic),
)

/** Secrets, one-time codes, keys: strings read or typed exactly. */
val JetBrainsMono: FontFamily = FontFamily(Font(R.font.jetbrains_mono, FontWeight(500)))
