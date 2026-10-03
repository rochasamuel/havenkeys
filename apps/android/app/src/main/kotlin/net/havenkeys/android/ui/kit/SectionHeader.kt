package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A group's title: sentence case, muted, a heading for TalkBack; text lines up with the rows' text. */
@Composable
fun SectionHeader(text: String, modifier: Modifier = Modifier, action: SectionAction? = null) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().heightIn(min = 36.dp).padding(start = HavenSpacing.rowX),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenText(
            text,
            Modifier.weight(1f).padding(vertical = 8.dp).semantics { heading() },
            style = HavenTheme.type.groupTitle,
            color = colors.muted,
        )
        if (action != null) {
            Box(
                Modifier
                    .defaultMinSize(minWidth = HavenSpacing.touch, minHeight = HavenSpacing.touch)
                    .clip(HavenShape.control)
                    .havenClickable(onClick = action.onClick)
                    .padding(horizontal = HavenSpacing.rowX),
                contentAlignment = Alignment.Center,
            ) {
                HavenText(action.label, style = HavenTheme.type.groupTitle, color = colors.brassInk)
            }
        }
    }
}

/** A trailing action on a header, such as "Clear" over recent searches. */
class SectionAction(val label: String, val onClick: () -> Unit)

@PreviewLightDark
@Composable
private fun SectionHeaderPreview() {
    KitPreview {
        SectionHeader("Frequently used")
        SectionHeader("Recent searches", action = SectionAction("Clear") {})
    }
}
