package net.havenkeys.android.ui.shell

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.layout.Box
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.theme.STAGGER_MILLIS

private val SettleShift = 8.dp

/**
 * Content that settles into place (spec §7: Home's groups after unlock, the
 * add sheet's tiles): it fades in and rises a few dp on the smooth spring,
 * [index] × 20 ms after it first appears. Under "Remove animations", or when
 * not [active], it is simply there. It takes taps from the first frame:
 * nothing waits for the animation.
 */
@Composable
fun Settle(index: Int, modifier: Modifier = Modifier, active: Boolean = true, content: @Composable () -> Unit) {
    val motion = HavenTheme.motion
    val shift = with(LocalDensity.current) { SettleShift.toPx() }
    val shown = remember { Animatable(if (!active || motion.reduced) 1f else 0f) }
    LaunchedEffect(shown) {
        if (shown.value < 1f) {
            delay(index * STAGGER_MILLIS.toLong())
            shown.animateTo(1f, motion.springSpec(HavenSprings.smooth))
        }
    }
    Box(
        modifier.graphicsLayer {
            alpha = shown.value
            translationY = (1f - shown.value) * shift
        },
    ) { content() }
}
