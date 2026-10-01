package net.havenkeys.android.ui.components

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.ContentCopy
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenColors
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.theme.HavenType

// Always the same count: the mask must not tell the value's length.
private const val MASK = "••••••••••••"

/**
 * One field of an item. The caller owns [revealed] (remembered state that
 * the lock wipe clears); this composable holds nothing. With [masked] the
 * field is a secret: dots until revealed, with a show/hide button. Without
 * it, [revealed] is a plain value shown as is.
 *
 * The value is never selectable (a system copy would skip the clipboard
 * clearing) and never part of a content description.
 */
@Composable
fun SecretField(
    label: String,
    revealed: String?,
    onReveal: () -> Unit,
    onCopy: () -> Unit,
    modifier: Modifier = Modifier,
    masked: Boolean = true,
) {
    val motion = HavenTheme.motion
    Row(
        modifier = modifier.padding(start = 16.dp, top = 8.dp, bottom = 8.dp, end = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (masked) {
                Crossfade(
                    targetState = revealed != null,
                    animationSpec = motion.snapSpec(),
                    label = "reveal",
                ) { shown ->
                    if (shown) {
                        Text(
                            text = colourised(revealed.orEmpty(), HavenTheme.colors),
                            style = HavenType.secret,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                    } else {
                        val hidden = stringResource(R.string.hidden, label)
                        Text(
                            text = MASK,
                            style = HavenType.masked,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            maxLines = 1,
                            modifier = Modifier.clearAndSetSemantics { contentDescription = hidden },
                        )
                    }
                }
            } else {
                Text(
                    text = revealed.orEmpty(),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )
            }
        }
        if (masked) {
            IconButton(onClick = onReveal) {
                if (revealed == null) {
                    Icon(Icons.Outlined.Visibility, contentDescription = stringResource(R.string.reveal, label))
                } else {
                    Icon(Icons.Outlined.VisibilityOff, contentDescription = stringResource(R.string.hide, label))
                }
            }
        }
        IconButton(onClick = onCopy) {
            Icon(Icons.Outlined.ContentCopy, contentDescription = stringResource(R.string.copy, label))
        }
    }
}

/** Digits and symbols in their own colours, as on the desktop, so 0/O and l/1 read apart. */
private fun colourised(value: String, colors: HavenColors): AnnotatedString = buildAnnotatedString {
    for (c in value) {
        when {
            c.isDigit() -> withStyle(SpanStyle(color = colors.digit)) { append(c) }
            !c.isLetter() && !c.isWhitespace() -> withStyle(SpanStyle(color = colors.symbol)) { append(c) }
            else -> append(c)
        }
    }
}

@Preview
@Composable
private fun SecretFieldPreview() {
    HavenTheme {
        Surface {
            Column {
                SecretField(
                    label = "Username",
                    revealed = "user@example.com",
                    onReveal = {},
                    onCopy = {},
                    masked = false,
                )
                SecretField(label = "Password", revealed = null, onReveal = {}, onCopy = {})
                SecretField(label = "Password", revealed = "not-a-real-password-42!", onReveal = {}, onCopy = {})
            }
        }
    }
}
