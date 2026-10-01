package net.havenkeys.android.ui.item

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

private const val FULL_TURN = 360f
private const val TOP = -90f

/**
 * How long the current one-time code has left: a ring that empties over the
 * period, stepping once a second (--dur-tick). Under "Remove animations" it
 * jumps instead of sweeping. Brass while there is time, danger in the last
 * five seconds.
 */
@Composable
fun TotpRing(secondsRemaining: Int, period: Int, modifier: Modifier = Modifier, size: Dp = 28.dp) {
    val motion = HavenTheme.motion
    val target = if (period > 0) secondsRemaining.toFloat() / period else 0f
    val progress by animateFloatAsState(target, motion.tickSpec(), label = "totp")
    val track = MaterialTheme.colorScheme.surfaceContainerHighest
    val arc = if (secondsRemaining <= ENDING_SECONDS) HavenTheme.colors.danger else HavenTheme.colors.brass
    val description = pluralStringResource(R.plurals.item_seconds_remaining, secondsRemaining, secondsRemaining)
    Canvas(modifier.size(size).semantics { contentDescription = description }) {
        val stroke = Stroke(width = 3.dp.toPx(), cap = StrokeCap.Round)
        drawArc(track, 0f, FULL_TURN, useCenter = false, style = stroke)
        drawArc(arc, TOP, FULL_TURN * progress, useCenter = false, style = stroke)
    }
}

private const val ENDING_SECONDS = 5

/** "381492" → "381 492", "12345678" → "1234 5678": easier to read and type. */
internal fun groupedCode(code: String): String =
    if (code.length < GROUP_MIN) code else code.substring(0, code.length / 2) + " " + code.substring(code.length / 2)

private const val GROUP_MIN = 6

@Preview
@Composable
private fun TotpRingPreview() {
    HavenTheme { TotpRing(secondsRemaining = 12, period = 30) }
}
