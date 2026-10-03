package net.havenkeys.android.ui.theme

import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Easing
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.InfiniteRepeatableSpec
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlin.math.PI

/** --ease-mechanical: a quick start that settles, like a bolt sliding home. */
val MechanicalEasing: Easing = CubicBezierEasing(0.3f, 0.7f, 0.2f, 1f)

/** --ease in apps/desktop/src/styles.css: the desktop's one curve. */
val HouseEasing: Easing = CubicBezierEasing(0.32f, 0.72f, 0f, 1f)

/** How far a pressed control shrinks (desktop: 0.97). */
const val PRESS_SCALE = 0.97f

/** The gap between items that arrive in sequence (sheet tiles, Home's groups). */
const val STAGGER_MILLIS = 30

/** --t-fast: fades, colour changes, the copy glyph. */
const val FADE_MILLIS = 160

/** One turn of the indeterminate ring. */
const val SPIN_MILLIS = 900

/**
 * A spring in Apple's terms (SwiftUI's response and damping fraction),
 * converted for Compose: stiffness = (2π / response)², dampingRatio = damping.
 */
@Immutable
data class HavenSpring(val dampingRatio: Float, val stiffness: Float) {
    companion object {
        fun of(responseSeconds: Float, damping: Float): HavenSpring {
            val omega = 2f * PI.toFloat() / responseSeconds
            return HavenSpring(dampingRatio = damping, stiffness = omega * omega)
        }
    }
}

/** The named springs of the kit. */
object HavenSprings {
    /** Pushes, tab changes, Home's groups settling, the search pill growing: no bounce. */
    val smooth = HavenSpring.of(responseSeconds = 0.5f, damping = 1f)

    /** Sheets and menus: a hint of settle. */
    val sheet = HavenSpring.of(responseSeconds = 0.4f, damping = 0.86f)

    /** The toast rising. */
    val toast = HavenSpring.of(responseSeconds = 0.35f, damping = 0.8f)

    /** Press scale, switch thumb, copy check: quick, no bounce. */
    val press = HavenSpring.of(responseSeconds = 0.2f, damping = 1f)
}

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

    /** A named spring, or an instant cut under "Remove animations". */
    fun <T> springSpec(kind: HavenSpring): FiniteAnimationSpec<T> =
        if (reduced) snap() else spring(dampingRatio = kind.dampingRatio, stiffness = kind.stiffness)

    /** --t-fast on the house curve: fades and colour changes. */
    fun <T> fadeSpec(): FiniteAnimationSpec<T> =
        if (reduced) snap() else tween(FADE_MILLIS, easing = HouseEasing)

    /**
     * One turn of a spinner, repeating. Callers draw a still arc instead when
     * [reduced] (a spinner under "Remove animations" would never stop moving).
     */
    fun spinSpec(): InfiniteRepeatableSpec<Float> =
        infiniteRepeatable(tween(SPIN_MILLIS, easing = LinearEasing), RepeatMode.Restart)

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
