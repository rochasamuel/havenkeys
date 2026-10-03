package net.havenkeys.android.credentials

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch

/**
 * Runs [block] only when the previous run has finished: a double tap on Save
 * starts one verification (one biometric prompt, one create), not two. Used
 * on the main thread only.
 */
internal class OneAtATime {
    var running = false
        private set

    /** Starts [block] in [scope] and returns true, or returns false while a run is still going. */
    fun launch(scope: CoroutineScope, block: suspend () -> Unit): Boolean {
        if (running) return false
        running = true
        scope.launch {
            try {
                block()
            } finally {
                running = false
            }
        }
        return true
    }
}
