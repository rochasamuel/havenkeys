package net.havenkeys.android.ui.theme

import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext

/** --ease-mechanical: a quick start that settles, like a bolt sliding home. */
val MechanicalEasing: Easing = CubicBezierEasing(0.3f, 0.7f, 0.2f, 1f)

/**
 * Durations from packages/ui/src/tokens.css. [reduced] is true when the
 * system's "Remove animations" sets the animator scale to 0; screens then
 * cut instead of moving (no shared element, no ring sweep).
 */
@Immutable
data class HavenMotion(
    /** --dur-snap: reveal, state changes, list → detail. */
    val snapMillis: Int,
    /** --dur-tick: the one-time code ring's step each second. */
    val tickMillis: Int,
    /** --dur-seal: unlock → vault and the lock wipe. */
    val sealMillis: Int,
    /** --dur-breath: the pulse while unlocking. */
    val breathMillis: Int,
    val reduced: Boolean,
) {
    fun <T> snapSpec(): FiniteAnimationSpec<T> = spec(snapMillis)

    fun <T> tickSpec(): FiniteAnimationSpec<T> = spec(tickMillis)

    fun <T> sealSpec(): FiniteAnimationSpec<T> = spec(sealMillis)

    private fun <T> spec(millis: Int): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(millis, easing = MechanicalEasing)
}

private val FullMotion = HavenMotion(
    snapMillis = 240,
    tickMillis = 250,
    sealMillis = 420,
    breathMillis = 1200,
    reduced = false,
)

private val ReducedMotion = HavenMotion(
    snapMillis = 0,
    tickMillis = 0,
    sealMillis = 0,
    breathMillis = 0,
    reduced = true,
)

fun havenMotion(animatorScale: Float): HavenMotion = if (animatorScale == 0f) ReducedMotion else FullMotion

/** Follows the system animator scale, including changes while the app is open. */
@Composable
internal fun rememberHavenMotion(): HavenMotion {
    val resolver = LocalContext.current.contentResolver
    val read = { Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) }
    var scale by remember { mutableFloatStateOf(read()) }
    DisposableEffect(resolver) {
        val observer = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) {
                scale = read()
            }
        }
        resolver.registerContentObserver(
            Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE),
            false,
            observer,
        )
        onDispose { resolver.unregisterContentObserver(observer) }
    }
    return havenMotion(scale)
}
