package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenRadius
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's inset group: a 12dp-rounded surface with a hairline border,
 * its rows separated by hairlines that start where the text of the row below
 * starts (past an item's tile or a row's glyph). Rows never get a card or
 * border of their own.
 */
@Composable
fun InsetGroup(modifier: Modifier = Modifier, content: InsetGroupScope.() -> Unit) {
    val colors = HavenTheme.colors
    val rows = InsetGroupScope().apply(content).rows
    Column(
        modifier
            .fillMaxWidth()
            .clip(HavenShape.group)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.group),
    ) {
        rows.forEachIndexed { index, row ->
            // A keyed row keeps its state (a field's focus, its typed text) when rows come and go before it.
            key(row.key ?: Position(index)) {
                val textStart = remember { RowTextStart() }
                if (index > 0) Hairline(textStart, colors.groupLine)
                CompositionLocalProvider(
                    LocalRowTextStart provides textStart,
                    LocalRowShape provides rowShape(first = index == 0, last = index == rows.lastIndex),
                ) { row.content() }
            }
        }
    }
}

@Composable
private fun Hairline(textStart: RowTextStart, color: Color) {
    val rtl = LocalLayoutDirection.current == LayoutDirection.Rtl
    Box(
        Modifier
            .fillMaxWidth()
            .height(1.dp)
            .drawBehind {
                val start = textStart.start.toPx().coerceAtMost(size.width)
                val x = if (rtl) 0f else start
                drawRect(color, Offset(x, 0f), Size(size.width - start, size.height))
            }
            .testTag(HAIRLINE_TAG)
            .semantics { hairlineStart = textStart.start },
    )
}

internal const val HAIRLINE_TAG = "inset-group-hairline"

/** Test seam: where a hairline starts, from the group's start edge. */
internal val HairlineStart = SemanticsPropertyKey<Dp>("HairlineStart")
private var SemanticsPropertyReceiver.hairlineStart by HairlineStart

/** Where the text of one row starts; the row says, the hairline above it follows. */
@Stable
internal class RowTextStart {
    var start: Dp by mutableStateOf(HavenSpacing.rowX)
}

internal val LocalRowTextStart = staticCompositionLocalOf<RowTextStart?> { null }

/**
 * The outline of the row being drawn: the group's rounded corners on its
 * first and last rows, square between. A row that draws its own ring (a
 * focused field) follows it, so the group's clip does not cut the ring.
 */
internal val LocalRowShape = staticCompositionLocalOf<Shape> { RectangleShape }

internal fun rowShape(first: Boolean, last: Boolean): Shape {
    val top = if (first) HavenRadius.group else 0.dp
    val bottom = if (last) HavenRadius.group else 0.dp
    return if (top == 0.dp && bottom == 0.dp) {
        RectangleShape
    } else {
        RoundedCornerShape(topStart = top, topEnd = top, bottomStart = bottom, bottomEnd = bottom)
    }
}

/** Tells an enclosing [InsetGroup] where this row's text starts (outside a group it does nothing). */
@Composable
internal fun ReportRowTextStart(start: Dp) {
    val textStart = LocalRowTextStart.current
    SideEffect { textStart?.start = start }
}

/** Collects an [InsetGroup]'s rows so the group can draw the hairlines between them. */
class InsetGroupScope internal constructor() {
    internal val rows = mutableListOf<GroupRowSlot>()

    /** A row; give it a [key] when rows before it are added or removed while it shows. */
    fun row(key: Any? = null, content: @Composable () -> Unit) {
        rows += GroupRowSlot(key, content)
    }
}

internal class GroupRowSlot(val key: Any?, val content: @Composable () -> Unit)

/** An unkeyed row's place, which no caller's key can equal. */
private data class Position(val index: Int)

@PreviewLightDark
@Composable
private fun InsetGroupPreview() {
    KitPreview {
        InsetGroup {
            row { GroupRow(onClick = {}) { GroupRowText("All items") } }
            row { GroupRow(onClick = {}) { GroupRowText("Logins") } }
        }
    }
}
