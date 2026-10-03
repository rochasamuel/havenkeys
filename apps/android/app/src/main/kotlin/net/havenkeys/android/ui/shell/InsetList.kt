package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.kit.LocalRowTextStart
import net.havenkeys.android.ui.kit.RowTextStart
import net.havenkeys.android.ui.theme.HavenRadius
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** How many corner radii the outline is drawn past an open edge, so the clipped corner never shows. */
private const val OVERDRAW = 2

/** Where a row sits in its group, which decides its corners and borders. */
@Suppress("MatchingDeclarationName")
internal enum class SlicePosition { Single, First, Middle, Last }

internal fun slicePosition(index: Int, count: Int): SlicePosition = when {
    count == 1 -> SlicePosition.Single
    index == 0 -> SlicePosition.First
    index == count - 1 -> SlicePosition.Last
    else -> SlicePosition.Middle
}

/**
 * The kit's InsetGroup for a lazy list: each row is a slice of one rounded,
 * hairline-bordered group, and the hairline above a row starts where that
 * row's own text starts (the row reports it, as in the kit's InsetGroup).
 * [key] must be unique in the whole list (prefix it when an item can be in
 * two groups); null keys rows by position. [around] wraps each slice, for
 * instance in a [Settle].
 */
fun <T> LazyListScope.insetGroup(
    items: List<T>,
    key: ((T) -> Any)?,
    around: @Composable (content: @Composable () -> Unit) -> Unit = { it() },
    row: @Composable (T) -> Unit,
) {
    itemsIndexed(items, key = key?.let { k -> { _: Int, item: T -> k(item) } }) { index, item ->
        around { InsetSlice(slicePosition(index, items.size)) { row(item) } }
    }
}

@Composable
internal fun InsetSlice(position: SlicePosition, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val colors = HavenTheme.colors
    val radius = HavenRadius.group
    val textStart = remember { RowTextStart() }
    val shape = when (position) {
        SlicePosition.Single -> HavenShape.group
        SlicePosition.First -> RoundedCornerShape(topStart = radius, topEnd = radius)
        SlicePosition.Last -> RoundedCornerShape(bottomStart = radius, bottomEnd = radius)
        SlicePosition.Middle -> RectangleShape
    }
    val opensTop = position == SlicePosition.Middle || position == SlicePosition.Last
    val opensBottom = position == SlicePosition.First || position == SlicePosition.Middle
    Box(
        modifier
            .padding(horizontal = HavenSpacing.gutter)
            .fillMaxWidth()
            .clip(shape)
            .background(colors.group)
            .drawWithContent {
                drawContent()
                val line = 1.dp.toPx()
                val r = radius.toPx()
                // The group's outline, drawn past this slice's open edges so only its own part shows.
                val top = if (opensTop) -OVERDRAW * r else line / 2
                val bottom = if (opensBottom) size.height + OVERDRAW * r else size.height - line / 2
                drawRoundRect(
                    color = colors.groupLine,
                    topLeft = Offset(line / 2, top),
                    size = Size(size.width - line, bottom - top),
                    cornerRadius = CornerRadius(r),
                    style = Stroke(line),
                )
                if (opensTop) {
                    val start = textStart.start.toPx().coerceAtMost(size.width)
                    drawLine(colors.groupLine, Offset(start, line / 2), Offset(size.width, line / 2), line)
                }
            },
    ) { CompositionLocalProvider(LocalRowTextStart provides textStart) { content() } }
}
