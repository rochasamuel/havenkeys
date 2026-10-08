package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

private val GlyphSize = 22.dp
private val GlyphGap = 14.dp

/** A row's end padding when a control or chevron ends it. */
private val TrailingEnd = 8.dp

/**
 * One row of an [InsetGroup]: an optional glyph, the row's text
 * ([GroupRowText] or [GroupRowField]), then [trailing] controls or, when it
 * opens something, a chevron. A tappable row is one button for TalkBack; a
 * read-only row's text (a label and its value) is one stop, and its trailing
 * controls stay their own.
 */
@Composable
fun GroupRow(
    modifier: Modifier = Modifier,
    onClick: (() -> Unit)? = null,
    onClickLabel: String? = null,
    icon: HavenIcon? = null,
    /** The glyph's colour; muted unless the glyph marks something (brass ink on a tag). */
    iconTint: Color? = null,
    trailing: (@Composable RowScope.() -> Unit)? = null,
    chevron: Boolean = onClick != null && trailing == null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val colors = HavenTheme.colors
    ReportRowTextStart(if (icon != null) HavenSpacing.rowX + GlyphSize + GlyphGap else HavenSpacing.rowX)
    val click = if (onClick == null) {
        Modifier
    } else {
        Modifier.havenClickable(onClickLabel = onClickLabel, onClick = onClick)
    }
    Row(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .then(click)
            .padding(
                start = HavenSpacing.rowX,
                end = if (trailing != null || chevron) TrailingEnd else HavenSpacing.rowX,
                top = 10.dp,
                bottom = 10.dp,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            IconGlyph(icon, contentDescription = null, tint = iconTint ?: colors.muted, size = GlyphSize)
            Spacer(Modifier.width(GlyphGap))
        }
        // Read-only: label and value are one TalkBack stop; Copy, Show and the like stay separate.
        val text = if (onClick == null) Modifier.semantics(mergeDescendants = true) {} else Modifier
        Column(
            Modifier.weight(1f).then(text),
            verticalArrangement = Arrangement.spacedBy(2.dp),
            content = content,
        )
        trailing?.invoke(this)
        if (chevron) {
            IconGlyph(
                HavenIcon.ChevronRight,
                contentDescription = null,
                Modifier.padding(start = 4.dp, end = 6.dp),
                tint = colors.muted,
                size = 18.dp,
            )
        }
    }
}

/** A row's text: a title, and an optional muted line under it. */
@Composable
fun GroupRowText(title: String, detail: String? = null) {
    HavenText(title, style = HavenTheme.type.value, color = HavenTheme.colors.textStrong)
    if (detail != null) HavenText(detail, style = HavenTheme.type.rowSubtitle, color = HavenTheme.colors.muted)
}

/** A field as an item's detail shows it: its label above its value. */
@Composable
fun GroupRowField(label: String, value: String, valueStyle: TextStyle = HavenTheme.type.value) {
    HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
    HavenText(value, style = valueStyle, color = HavenTheme.colors.textStrong)
}

/**
 * A muted value at a row's end: a count, the current setting. A row with
 * trailing content keeps only 8dp at its end (room a 48dp control fills), so
 * the value adds the rest and sits on the same 16dp margin as the row's text.
 */
@Composable
fun TrailingText(text: String) {
    HavenText(
        text,
        Modifier.padding(end = HavenSpacing.rowX - TrailingEnd),
        style = HavenTheme.type.value,
        color = HavenTheme.colors.muted,
    )
}

@PreviewLightDark
@Composable
private fun GroupRowPreview() {
    KitPreview {
        InsetGroup {
            row { GroupRow(onClick = {}, icon = HavenIcon.Clock) { GroupRowText("Auto-lock", "After 5 minutes") } }
            row { GroupRow(onClick = {}, trailing = { TrailingText("12") }) { GroupRowText("Logins") } }
            row {
                GroupRow(trailing = { CopyButton("Username", onCopy = {}) }) {
                    GroupRowField("Username", "sam@example.com")
                }
            }
        }
    }
}
