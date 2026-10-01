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

    fun show(v: String) {
        value = v
        timer?.cancel()
        timer = scope.launch {
            delay(REVEAL_MS)
            value = null
        }
    }

    fun clear() {
        timer?.cancel()
        value = null
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
