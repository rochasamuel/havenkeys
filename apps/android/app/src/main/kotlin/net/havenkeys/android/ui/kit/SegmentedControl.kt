package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenRadius
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's segmented control: a field track with the chosen segment
 * raised on a hairline. Each segment is a tab for TalkBack and a full 48dp
 * target; the track is drawn inset inside that height. In light theme the
 * track is the hover green (the field is white there, like the pane).
 */
@Composable
fun SegmentedControl(
    options: List<String>,
    selectedIndex: Int,
    onSelect: (Int) -> Unit,
    modifier: Modifier = Modifier,
) {
    val colors = HavenTheme.colors
    val track = if (colors.isDark) colors.field else colors.hover
    Row(
        modifier
            .fillMaxWidth()
            .selectableGroup()
            .drawBehind {
                val inset = 4.dp.toPx()
                drawRoundRect(
                    color = track,
                    topLeft = Offset(0f, inset),
                    size = Size(size.width, size.height - 2 * inset),
                    cornerRadius = CornerRadius(HavenRadius.row.toPx()),
                )
            }
            .padding(horizontal = 3.dp),
    ) {
        options.forEachIndexed { index, option ->
            val selected = index == selectedIndex
            val thumb by animateFloatAsState(if (selected) 1f else 0f, HavenTheme.motion.fadeSpec(), label = "segment")
            Box(
                Modifier
                    .weight(1f)
                    .heightIn(min = HavenSpacing.touch)
                    .selectable(
                        selected = selected,
                        interactionSource = null,
                        indication = null,
                        role = Role.Tab,
                    ) { onSelect(index) }
                    .drawBehind { drawSegmentThumb(colors.raised, colors.lineStrong, thumb) }
                    .padding(horizontal = 10.dp, vertical = 12.dp),
                contentAlignment = Alignment.Center,
            ) {
                HavenText(
                    option,
                    style = HavenTheme.type.label,
                    color = if (selected) colors.textStrong else colors.muted,
                )
            }
        }
    }
}

private fun DrawScope.drawSegmentThumb(fill: Color, line: Color, amount: Float) {
    if (amount <= 0f) return
    val inset = 7.dp.toPx()
    val topLeft = Offset(0f, inset)
    val area = Size(size.width, size.height - 2 * inset)
    val corner = CornerRadius(HavenRadius.control.toPx())
    drawRoundRect(fill, topLeft, area, corner, alpha = amount)
    drawRoundRect(line, topLeft, area, corner, style = Stroke(1.dp.toPx()), alpha = amount)
}

@PreviewLightDark
@Composable
private fun SegmentedControlPreview() {
    KitPreview { SegmentedControl(listOf("Random", "Words", "PIN"), selectedIndex = 1, onSelect = {}) }
}
