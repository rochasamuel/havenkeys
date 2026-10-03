package net.havenkeys.android.ui.kit

import android.graphics.Paint
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.selection.toggleable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The desktop's switch at phone size: line-strong track off, brass on, a
 * paper-white thumb. With [onCheckedChange] it is its own control (give it
 * a [label]); with null it is drawn only, inside a [ToggleRow] that toggles.
 */
@Composable
fun HavenSwitch(
    checked: Boolean,
    onCheckedChange: ((Boolean) -> Unit)?,
    modifier: Modifier = Modifier,
    label: String? = null,
    enabled: Boolean = true,
) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val position by animateFloatAsState(if (checked) 1f else 0f, motion.springSpec(HavenSprings.press), label = "thumb")
    val track by animateColorAsState(
        if (checked) colors.brass else colors.lineStrong,
        motion.fadeSpec(),
        label = "track",
    )
    val control = if (onCheckedChange == null) {
        Modifier
    } else {
        Modifier
            .toggleable(
                value = checked,
                interactionSource = null,
                indication = null,
                enabled = enabled,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            .semantics { if (label != null) contentDescription = label }
    }
    Box(
        modifier
            .then(control)
            .defaultMinSize(minWidth = 52.dp, minHeight = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA),
        contentAlignment = Alignment.Center,
    ) {
        Canvas(Modifier.size(width = 44.dp, height = 26.dp)) {
            val radius = size.height / 2
            drawRoundRect(track, cornerRadius = CornerRadius(radius))
            val thumb = radius - 3.dp.toPx()
            val x = radius + (size.width - 2 * radius) * position
            drawThumb(colors.thumb, thumb, Offset(x, radius), shadowAlpha = 0.35f, blur = 3.dp)
        }
    }
}

/** Padding after the switch box so its track ends on the same margin as row text. */
internal val TOGGLE_END = HavenSpacing.rowX - 4.dp

/** A settings row with a switch: the whole row is the switch, named by its title. */
@Composable
fun ToggleRow(
    title: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    detail: String? = null,
    enabled: Boolean = true,
) {
    Row(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .toggleable(
                value = checked,
                interactionSource = null,
                indication = HavenPress,
                enabled = enabled,
                role = Role.Switch,
                onValueChange = onCheckedChange,
            )
            // The switch's box is 4dp wider than its track on each side: 12 + 4 = the 16dp row margin.
            .padding(start = HavenSpacing.rowX, end = TOGGLE_END, top = 10.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) { GroupRowText(title, detail) }
        HavenSwitch(checked, onCheckedChange = null, enabled = enabled)
    }
}

/**
 * A paper-white thumb with the desktop's soft shadow (styles.css: 0 1px 3px
 * on the switch, 0 1px 4px on the slider): offset 1dp down and blurred, so it
 * rings the whole thumb and lifts it off a white ground too.
 */
internal fun DrawScope.drawThumb(fill: Color, radius: Float, center: Offset, shadowAlpha: Float, blur: Dp) {
    val paint = Paint().apply {
        isAntiAlias = true
        color = fill.toArgb()
        setShadowLayer(blur.toPx() / 2, 0f, 1.dp.toPx(), Color.Black.copy(alpha = shadowAlpha).toArgb())
    }
    drawIntoCanvas { it.nativeCanvas.drawCircle(center.x, center.y, radius, paint) }
}

@PreviewLightDark
@Composable
private fun HavenSwitchPreview() {
    KitPreview {
        InsetGroup {
            row { ToggleRow("Unlock with biometrics", checked = true, onCheckedChange = {}) }
            row {
                ToggleRow(
                    "Confirm before filling",
                    checked = false,
                    onCheckedChange = {},
                    detail = "Ask before each fill",
                )
            }
        }
    }
}
