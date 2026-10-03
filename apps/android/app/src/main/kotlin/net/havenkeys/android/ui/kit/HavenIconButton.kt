package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.isSpecified
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** A glyph that is a button: 48dp round target, muted unless [tint] says otherwise. */
@Composable
fun HavenIconButton(
    icon: HavenIcon,
    contentDescription: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    tint: Color = Color.Unspecified,
) {
    val ink = if (tint.isSpecified) tint else HavenTheme.colors.muted
    Box(
        modifier = modifier
            .size(HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .clip(CircleShape)
            .havenClickable(enabled = enabled, onClick = onClick)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        IconGlyph(icon, contentDescription = null, tint = ink, size = 22.dp)
    }
}

@PreviewLightDark
@Composable
private fun HavenIconButtonPreview() {
    KitPreview {
        Row {
            HavenIconButton(HavenIcon.Lock, "Lock", onClick = {})
            HavenIconButton(HavenIcon.More, "More", onClick = {})
            HavenIconButton(HavenIcon.Trash, "Delete", onClick = {}, tint = HavenTheme.colors.danger)
        }
    }
}
