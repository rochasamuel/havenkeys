package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Disabled controls fade to this opacity (desktop: 42%). */
internal const val DISABLED_ALPHA = 0.42f

/**
 * A button. One [ButtonStyle.Primary] per screen: brass in dark, forest in
 * light (the desktop's One Fitting Rule). Secondary is outlined, quiet is
 * bare text, danger fills ember. The label wraps; it is never cut.
 */
@Composable
fun HavenButton(
    text: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    style: ButtonStyle = ButtonStyle.Primary,
    enabled: Boolean = true,
    icon: HavenIcon? = null,
) {
    val colors = HavenTheme.colors
    val ground = when (style) {
        ButtonStyle.Primary -> colors.primary
        ButtonStyle.Danger -> colors.danger
        ButtonStyle.Secondary, ButtonStyle.Quiet -> Color.Transparent
    }
    val ink = when (style) {
        ButtonStyle.Primary -> colors.onPrimary
        ButtonStyle.Danger -> colors.onDanger
        ButtonStyle.Secondary, ButtonStyle.Quiet -> colors.textStrong
    }
    val edge = if (style == ButtonStyle.Secondary) colors.lineStrong else Color.Transparent
    Row(
        modifier = modifier
            .defaultMinSize(minWidth = HavenSpacing.touch, minHeight = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(HavenShape.row)
            .havenClickable(enabled = enabled, onClick = onClick)
            .background(ground, HavenShape.row)
            .border(1.dp, edge, HavenShape.row)
            .padding(horizontal = 18.dp, vertical = 12.dp),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) {
            IconGlyph(icon, contentDescription = null, tint = ink, size = 18.dp)
            Spacer(Modifier.width(8.dp))
        }
        HavenText(text, style = HavenTheme.type.button.copy(textAlign = TextAlign.Center), color = ink)
    }
}

enum class ButtonStyle { Primary, Secondary, Quiet, Danger }

@PreviewLightDark
@Composable
private fun HavenButtonPreview() {
    KitPreview {
        HavenButton("Unlock", onClick = {}, modifier = Modifier.fillMaxWidth())
        HavenButton("Generate", onClick = {}, style = ButtonStyle.Secondary, icon = HavenIcon.Dice)
        HavenButton("Not now", onClick = {}, style = ButtonStyle.Quiet)
        HavenButton("Remove this device", onClick = {}, style = ButtonStyle.Danger)
        HavenButton("Disabled", onClick = {}, enabled = false)
    }
}
