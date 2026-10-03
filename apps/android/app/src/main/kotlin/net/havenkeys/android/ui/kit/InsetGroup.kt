package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
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
import androidx.compose.ui.platform.LocalLayoutDirection
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.SemanticsPropertyReceiver
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
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
            val textStart = remember { RowTextStart() }
            if (index > 0) Hairline(textStart, colors.groupLine)
            CompositionLocalProvider(LocalRowTextStart provides textStart) { row() }
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

/** Tells an enclosing [InsetGroup] where this row's text starts (outside a group it does nothing). */
@Composable
internal fun ReportRowTextStart(start: Dp) {
    val textStart = LocalRowTextStart.current
    SideEffect { textStart?.start = start }
}

/** Collects an [InsetGroup]'s rows so the group can draw the hairlines between them. */
class InsetGroupScope internal constructor() {
    internal val rows = mutableListOf<@Composable () -> Unit>()

    fun row(content: @Composable () -> Unit) {
        rows += content
    }
}

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
