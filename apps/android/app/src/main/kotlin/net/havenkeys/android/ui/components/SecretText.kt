package net.havenkeys.android.ui.components

import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
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

/**
 * A hidden value: the fixed row of dots; TalkBack hears "Hidden <label>", never the value or its length.
 * Where the row already reads its label next to the dots, [label] is null and TalkBack hears only
 * "Hidden", so the label is not read twice ("Password, Hidden").
 */
@Composable
fun MaskedValue(label: String?, modifier: Modifier = Modifier) {
    val hidden = if (label != null) stringResource(R.string.hidden, label) else stringResource(R.string.hidden_value)
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

/**
 * A partly masked value as every screen draws it (`•••45-6`, `•••• 1234`):
 * each run of dots in the house masked style, in [dots] colour, and the
 * rest in the surrounding text style. Mono is only for revealed values.
 */
@Composable
fun maskedSpans(text: String, dots: Color = Color.Unspecified): AnnotatedString {
    val masked = HavenTheme.type.masked
    val style = SpanStyle(
        color = dots,
        fontFamily = masked.fontFamily,
        fontWeight = masked.fontWeight,
        letterSpacing = masked.letterSpacing,
    )
    return buildAnnotatedString {
        DOT_RUNS.findAll(text).forEach { run ->
            if (run.value[0] == '•') withStyle(style) { append(run.value) } else append(run.value)
        }
    }
}

private val DOT_RUNS = Regex("•+|[^•]+")

/**
 * A hidden value that shows its ending ([mask] is `•••45-6`): muted dots,
 * the ending as a value. TalkBack hears [description] ("Account ending 45-6").
 */
@Composable
fun MaskedEnding(mask: String, description: String, modifier: Modifier = Modifier) {
    HavenText(
        maskedSpans(mask, dots = HavenTheme.colors.muted),
        modifier.clearAndSetSemantics { contentDescription = description },
        style = HavenTheme.type.value,
        color = HavenTheme.colors.textStrong,
        maxLines = 1,
    )
}
