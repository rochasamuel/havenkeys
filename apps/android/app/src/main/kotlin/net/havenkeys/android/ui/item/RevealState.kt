package net.havenkeys.android.ui.item

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * One revealed value, held only by the composable showing it (spec §9.4):
 * cleared after 30 seconds, when it leaves the screen, when the app goes to
 * the background, and on lock (the screen is popped). Never in a ViewModel,
 * never saved.
 */
class RevealState(private val scope: CoroutineScope) {
    var value: String? by mutableStateOf(null)
        private set
    private var timer: Job? = null

    /**
     * Bumped by every [clear]. A reveal asked of Rust before a clear (ON_STOP, leave, a tap on hide)
     * must not show its answer after it: the caller notes this before asking and shows only if unchanged.
     */
    var generation: Int = 0
        private set

    fun show(v: String) {
        value = v
        timer?.cancel()
        timer = scope.launch {
            delay(REVEAL_MS)
            value = null
        }
    }

    fun clear() {
        generation++
        timer?.cancel()
        value = null
    }

    /** Shows [v] only if nothing cleared this state since [asked] was read from [generation]. */
    fun showIfCurrent(asked: Int, v: String) {
        if (asked == generation) show(v)
    }

    companion object {
        const val REVEAL_MS = 30_000L
    }
}

@Composable
fun rememberRevealState(): RevealState {
    val scope = rememberCoroutineScope()
    val state = remember { RevealState(scope) }
    DisposableEffect(state) { onDispose { state.clear() } }
    LifecycleEventEffect(Lifecycle.Event.ON_STOP) { state.clear() }
    return state
}
