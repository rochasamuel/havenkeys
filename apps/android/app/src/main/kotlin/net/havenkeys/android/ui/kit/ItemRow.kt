package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import java.text.BreakIterator
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Test seam: marks the glyph a tile draws, so a test can tell the blank-title key from an initial. */
internal fun tileGlyphTag(icon: HavenIcon): String = "item-tile-glyph-${icon.name}"

private const val MONOGRAM_RATIO = 0.45f

/** Between an item row's tile and its text. */
private val TileGap = 12.dp

/**
 * One vault item in a list: tile, title, non-secret subtitle, and marks for
 * a passkey and a one-time code. One button for TalkBack; the tile is
 * decoration. [titleModifier] and [tileModifier] let the title and the tile travel
 * to the detail screen. An optional [note] is a muted line of our own under
 * the subtitle (the Trash's "Deleted 3 days ago"); it wraps.
 * Only the title and subtitle (user data) may be cut with an ellipsis.
 */
@Composable
fun ItemRow(
    title: String,
    subtitle: String?,
    leading: RowLeading,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    titleModifier: Modifier = Modifier,
    tileModifier: Modifier = Modifier,
    hasPasskey: Boolean = false,
    hasCode: Boolean = false,
    note: String? = null,
) {
    val colors = HavenTheme.colors
    ReportRowTextStart(HavenSpacing.rowX + HavenSpacing.tile + TileGap)
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.itemRowMin)
            .havenClickable(onClick = onClick)
            .padding(horizontal = HavenSpacing.rowX, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ItemTile(leading, tileModifier)
        Spacer(Modifier.width(TileGap))
        Column(Modifier.weight(1f)) {
            HavenText(
                title,
                titleModifier,
                style = HavenTheme.type.rowTitle,
                color = colors.textStrong,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (subtitle != null) {
                HavenText(
                    subtitle,
                    style = HavenTheme.type.rowSubtitle,
                    color = colors.muted,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (note != null) HavenText(note, style = HavenTheme.type.rowSubtitle, color = colors.muted)
        }
        if (hasCode) {
            IconGlyph(
                HavenIcon.Clock,
                stringResource(R.string.field_totp),
                Modifier.padding(start = 8.dp),
                tint = colors.muted,
                size = 16.dp,
            )
        }
        if (hasPasskey) {
            IconGlyph(
                HavenIcon.Key,
                stringResource(R.string.item_passkey),
                Modifier.padding(start = 8.dp),
                tint = colors.muted,
                size = 16.dp,
            )
        }
    }
}

/** What stands at the start of an item row: the title's initial, or the kind's glyph. */
sealed interface RowLeading {
    data class Monogram(val title: String) : RowLeading

    /** [soft]: on the brass wash, as the desktop draws secure notes. */
    data class Glyph(val icon: HavenIcon, val soft: Boolean = false) : RowLeading
}

/**
 * The tile: a serif initial on the avatar ground, or a glyph. A blank title
 * shows the key. The initial keeps its size at any system font size, so it
 * never spills out of the tile.
 */
@Composable
fun ItemTile(leading: RowLeading, modifier: Modifier = Modifier, size: Dp = HavenSpacing.tile) {
    val colors = HavenTheme.colors
    val initial = (leading as? RowLeading.Monogram)?.let { monogramOf(it.title) }
    val glyph = when {
        leading is RowLeading.Glyph -> leading
        initial == null -> RowLeading.Glyph(HavenIcon.Key)
        else -> null
    }
    val soft = glyph?.soft == true
    Box(
        modifier
            .size(size)
            .clip(RoundedCornerShape(size / 4))
            .background(if (soft) colors.brassSoft else colors.avatarBg)
            .clearAndSetSemantics {},
        contentAlignment = Alignment.Center,
    ) {
        if (glyph != null) {
            IconGlyph(
                glyph.icon,
                contentDescription = null,
                Modifier.testTag(tileGlyphTag(glyph.icon)),
                tint = if (soft) colors.brassInk else colors.avatarFg,
                size = size / 2,
            )
        } else {
            val fontSize = with(LocalDensity.current) { (size * MONOGRAM_RATIO).toSp() }
            HavenText(
                initial.orEmpty(),
                style = HavenTheme.type.monogram.copy(fontSize = fontSize, lineHeight = fontSize),
                color = colors.avatarFg,
            )
        }
    }
}

/** The first character as a reader sees it (a whole emoji), upper-cased unless that changes its length ("ß"). */
internal fun monogramOf(title: String): String? {
    val text = title.trim()
    if (text.isEmpty()) return null
    val breaks = BreakIterator.getCharacterInstance().apply { setText(text) }
    val first = text.substring(0, breaks.next())
    val upper = first.uppercase()
    return if (upper.length == first.length) upper else first
}

@PreviewLightDark
@Composable
private fun ItemRowPreview() {
    KitPreview {
        InsetGroup {
            row {
                ItemRow(
                    "GitHub",
                    "sam@example.com",
                    RowLeading.Monogram("GitHub"),
                    onClick = {},
                    hasPasskey = true,
                    hasCode = true,
                )
            }
            row { ItemRow("Wi-Fi at home", "Secure note", RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
            row { ItemRow("Visa ending 4242", "Card", RowLeading.Glyph(HavenIcon.Card), onClick = {}) }
        }
    }
}
