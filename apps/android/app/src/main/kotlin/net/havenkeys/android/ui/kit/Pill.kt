package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

/** A small marker. Not a control: it has no action and needs no touch target. */
@Composable
fun Pill(text: String, modifier: Modifier = Modifier, tone: PillTone = PillTone.Brass) {
    val colors = HavenTheme.colors
    val ground = if (tone == PillTone.Brass) colors.brassSoft else Color.Transparent
    val edge = if (tone == PillTone.Outline) colors.lineStrong else Color.Transparent
    HavenText(
        text,
        modifier
            .clip(HavenShape.pill)
            .background(ground)
            .border(1.dp, edge, HavenShape.pill)
            .padding(horizontal = 9.dp, vertical = 3.dp),
        style = HavenTheme.type.pill,
        color = if (tone == PillTone.Brass) colors.brassInk else colors.muted,
    )
}

enum class PillTone {
    /** Brass wash, brass ink: a state or a device marker ("This device"). */
    Brass,

    /** Outlined, muted: how something applies ("Whole site"). */
    Outline,
}

@PreviewLightDark
@Composable
private fun PillPreview() {
    KitPreview {
        Row {
            Pill("This device")
            Pill("Whole site", Modifier.padding(start = 8.dp), tone = PillTone.Outline)
        }
    }
}
