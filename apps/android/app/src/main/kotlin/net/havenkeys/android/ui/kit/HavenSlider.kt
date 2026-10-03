package net.havenkeys.android.ui.kit

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlin.math.roundToInt
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

private val Thumb = 24.dp

/**
 * The desktop's slider: brass fill on a line-strong track, a paper-white
 * thumb. Tap or drag anywhere on its 48dp height; TalkBack adjusts it with
 * its own gestures (setProgress), snapped to [steps]. [valueText] is what
 * TalkBack reads as the value (a length reads "24", not a percentage).
 */
@Composable
fun HavenSlider(
    value: Float,
    onValueChange: (Float) -> Unit,
    valueRange: ClosedFloatingPointRange<Float>,
    label: String,
    modifier: Modifier = Modifier,
    steps: Int = 0,
    valueText: String? = null,
    enabled: Boolean = true,
) {
    val colors = HavenTheme.colors
    val latest by rememberUpdatedState(onValueChange)
    val thumbPx = with(LocalDensity.current) { Thumb.toPx() }
    var trackPx by remember { mutableFloatStateOf(1f) }
    val span = valueRange.endInclusive - valueRange.start
    fun valueAt(x: Float): Float {
        val fraction = ((x - thumbPx / 2) / trackPx).coerceIn(0f, 1f)
        return snapToStep(valueRange.start + fraction * span, valueRange, steps)
    }
    val fraction = if (span > 0f) ((value - valueRange.start) / span).coerceIn(0f, 1f) else 0f
    Box(
        modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.touch)
            .alpha(if (enabled) 1f else DISABLED_ALPHA)
            .onSizeChanged { trackPx = (it.width - thumbPx).coerceAtLeast(1f) }
            .pointerInput(enabled, valueRange, steps) {
                if (enabled) detectTapGestures { latest(valueAt(it.x)) }
            }
            .pointerInput(enabled, valueRange, steps) {
                if (enabled) {
                    detectHorizontalDragGestures { change, _ ->
                        change.consume()
                        latest(valueAt(change.position.x))
                    }
                }
            }
            .semantics {
                contentDescription = label
                if (valueText != null) stateDescription = valueText
                progressBarRangeInfo = ProgressBarRangeInfo(value, valueRange, steps)
                if (enabled) {
                    setProgress { target ->
                        latest(snapToStep(target, valueRange, steps))
                        true
                    }
                } else {
                    disabled()
                }
            },
        contentAlignment = Alignment.CenterStart,
    ) {
        Canvas(Modifier.fillMaxWidth().height(Thumb)) {
            val y = size.height / 2
            val start = thumbPx / 2
            val end = size.width - thumbPx / 2
            val x = start + (end - start) * fraction
            val stroke = 4.dp.toPx()
            drawLine(colors.lineStrong, Offset(start, y), Offset(end, y), strokeWidth = stroke, cap = StrokeCap.Round)
            drawLine(colors.brass, Offset(start, y), Offset(x, y), strokeWidth = stroke, cap = StrokeCap.Round)
            drawThumb(colors.thumb, thumbPx / 2, Offset(x, y), shadowAlpha = 0.45f, blur = 4.dp)
        }
    }
}

/** [value] in [range], on the nearest of [steps] evenly spaced stops (0: continuous). */
internal fun snapToStep(value: Float, range: ClosedFloatingPointRange<Float>, steps: Int): Float {
    val clamped = value.coerceIn(range)
    if (steps <= 0) return clamped
    val step = (range.endInclusive - range.start) / (steps + 1)
    return (range.start + ((clamped - range.start) / step).roundToInt() * step).coerceIn(range)
}

@PreviewLightDark
@Composable
private fun HavenSliderPreview() {
    KitPreview { HavenSlider(24f, {}, 8f..64f, label = "Length", steps = 55, valueText = "24") }
}
