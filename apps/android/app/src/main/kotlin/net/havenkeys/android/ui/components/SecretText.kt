package net.havenkeys.android.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.theme.HavenColors
import net.havenkeys.android.ui.theme.HavenTheme

// Always the same count: the mask must not tell the value's length.
internal const val MASK = "••••••••••••"

/** Digits and symbols in their own colours, as on the desktop, so 0/O and l/1 read apart. */
internal fun colourised(value: String, colors: HavenColors): AnnotatedString = buildAnnotatedString {
    for (c in value) {
        when {
            c.isDigit() -> withStyle(SpanStyle(color = colors.digit)) { append(c) }
            !c.isLetter() && !c.isWhitespace() -> withStyle(SpanStyle(color = colors.symbol)) { append(c) }
            else -> append(c)
        }
    }
}

/** A hidden value: the fixed row of dots; TalkBack hears "Hidden <label>", never the value or its length. */
@Composable
fun MaskedValue(label: String, modifier: Modifier = Modifier) {
    val hidden = stringResource(R.string.hidden, label)
    HavenText(
        MASK,
        modifier.clearAndSetSemantics { contentDescription = hidden },
        style = HavenTheme.type.masked,
        color = HavenTheme.colors.muted,
        maxLines = 1,
    )
}

/**
 * A revealed secret in mono, digits and symbols coloured. Never selectable:
 * a system copy would skip the clipboard's clearing.
 */
@Composable
fun RevealedValue(value: String, modifier: Modifier = Modifier, style: TextStyle = HavenTheme.type.secret) {
    HavenText(
        colourised(value, HavenTheme.colors),
        modifier,
        style = style,
        color = HavenTheme.colors.textStrong,
    )
}
