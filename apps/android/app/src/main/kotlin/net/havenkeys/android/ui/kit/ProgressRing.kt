package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenTheme

private const val SWEEP = 270f
private const val TOP = -90f

/**
 * A ring: [progress] 0..1 drains or fills it on the tick spec (the one-time
 * code's ring); null spins it while something is working. Brass, or ember
 * when [warn]. Under "Remove animations" it steps and does not spin.
 */
@Composable
fun ProgressRing(
    progress: Float?,
    modifier: Modifier = Modifier,
    size: Dp = 28.dp,
    warn: Boolean = false,
    contentDescription: String? = null,
) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val arc = if (warn) colors.danger else colors.brass
    val shown by animateFloatAsState((progress ?: 0f).coerceIn(0f, 1f), motion.tickSpec(), label = "progress")
    val spin = if (progress == null && !motion.reduced) {
        rememberInfiniteTransition(label = "spin").animateFloat(
            initialValue = 0f,
            targetValue = 360f,
            animationSpec = motion.spinSpec(),
            label = "angle",
        )
    } else {
        null
    }
    Canvas(
        modifier.size(size).semantics {
            progressBarRangeInfo = if (progress == null) {
                ProgressBarRangeInfo.Indeterminate
            } else {
                ProgressBarRangeInfo(progress.coerceIn(0f, 1f), 0f..1f)
            }
            if (contentDescription != null) this.contentDescription = contentDescription
        },
    ) {
        val stroke = Stroke(width = 3.dp.toPx(), cap = StrokeCap.Round)
        val inset = stroke.width / 2
        val box = Size(this.size.width - stroke.width, this.size.height - stroke.width)
        drawArc(colors.line, 0f, 360f, false, Offset(inset, inset), box, style = stroke)
        if (progress == null) {
            drawArc(arc, TOP + (spin?.value ?: 0f), SWEEP, false, Offset(inset, inset), box, style = stroke)
        } else {
            drawArc(arc, TOP, 360f * shown, false, Offset(inset, inset), box, style = stroke)
        }
    }
}

@PreviewLightDark
@Composable
private fun ProgressRingPreview() {
    KitPreview {
        Row {
            ProgressRing(0.7f)
            ProgressRing(0.12f, warn = true)
            ProgressRing(null)
        }
    }
}
