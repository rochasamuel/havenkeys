package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's inset group: a 12dp-rounded surface with a hairline border,
 * its rows separated by hairlines that start where the rows' text starts.
 * Rows never get a card or border of their own.
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
            if (index > 0) {
                Box(
                    Modifier
                        .padding(start = HavenSpacing.rowX)
                        .fillMaxWidth()
                        .height(1.dp)
                        .background(colors.groupLine),
                )
            }
            row()
        }
    }
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
