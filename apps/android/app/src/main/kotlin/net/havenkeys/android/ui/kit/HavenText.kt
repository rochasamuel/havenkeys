package net.havenkeys.android.ui.kit

import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextOverflow
import net.havenkeys.android.ui.theme.HavenTheme

/** The colour text and glyphs take when the caller names none (foundation has no content colour). */
val LocalHavenContentColor = compositionLocalOf { Color.Unspecified }

@Composable
fun ProvideContentColor(color: Color, content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalHavenContentColor provides color, content = content)
}

@Composable
internal fun contentColor(): Color {
    val provided = LocalHavenContentColor.current
    return if (provided.isSpecified) provided else HavenTheme.colors.text
}

/**
 * Text in the kit. Our own words wrap and are never cut; only user data
 * (titles, usernames, URLs) passes `overflow = TextOverflow.Ellipsis`.
 */
@Composable
fun HavenText(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = HavenTheme.type.body,
    color: Color = Color.Unspecified,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val ink = if (color.isSpecified) color else contentColor()
    BasicText(text, modifier, style.copy(color = ink), overflow = overflow, maxLines = maxLines)
}

/** Styled text, such as a generated password with coloured digits and symbols. */
@Composable
fun HavenText(
    text: AnnotatedString,
    modifier: Modifier = Modifier,
    style: TextStyle = HavenTheme.type.body,
    color: Color = Color.Unspecified,
    maxLines: Int = Int.MAX_VALUE,
    overflow: TextOverflow = TextOverflow.Clip,
) {
    val ink = if (color.isSpecified) color else contentColor()
    BasicText(text, modifier, style.copy(color = ink), overflow = overflow, maxLines = maxLines)
}
