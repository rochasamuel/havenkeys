package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A small marker. Not a control: it has no action and needs no touch target.
 * An optional [icon] (decoration, 12dp in the pill's ink) says what kind of
 * marker it is, as the tag glyph does on an item's tags.
 */
@Composable
fun Pill(text: String, modifier: Modifier = Modifier, tone: PillTone = PillTone.Brass, icon: HavenIcon? = null) {
    val colors = HavenTheme.colors
    val ground = if (tone == PillTone.Brass) colors.brassSoft else Color.Transparent
    val edge = if (tone == PillTone.Outline) colors.lineStrong else Color.Transparent
    val ink = if (tone == PillTone.Brass) colors.brassInk else colors.muted
    Row(
        modifier
            .clip(HavenShape.pill)
            .background(ground)
            .border(1.dp, edge, HavenShape.pill)
            .padding(start = if (icon != null) 7.dp else 9.dp, end = 9.dp, top = 3.dp, bottom = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (icon != null) IconGlyph(icon, contentDescription = null, tint = ink, size = 12.dp)
        HavenText(text, style = HavenTheme.type.pill, color = ink)
    }
}

enum class PillTone {
    /** Brass wash, brass ink: a state or a device marker ("This device"), and an item's tags. */
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
            Pill("Work", Modifier.padding(start = 8.dp), icon = HavenIcon.Tag)
        }
    }
}
