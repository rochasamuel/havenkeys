package net.havenkeys.android.data

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async

/**
 * Runs one job at a time, in a scope the coalescer owns: every caller, the
 * first included, only waits for the shared run's outcome, so cancelling a
 * caller (a cleared ViewModel) stops its own wait and never the run or the
 * others' result. Sync now and pull to refresh share one in-flight sync this way.
 */
class Coalescer<T>(private val scope: CoroutineScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)) {
    private var running: Deferred<Outcome<T>>? = null

    /**
     * A caller that arrives while a run is in flight joins it. With [fresh] it
     * first waits for that run to end and then joins or starts one that began
     * after the call, for callers that need what happened up to now (an edit
     * conflict needs the other device's newer version, which a sync already
     * under way may have missed).
     */
    suspend fun run(fresh: Boolean = false, job: suspend () -> Outcome<T>): Outcome<T> {
        if (fresh) synchronized(this) { running }?.await()
        val start = synchronized(this) {
            running?.let { return@synchronized it to false }
            val flight = scope.async(start = CoroutineStart.LAZY) {
                try {
                    job()
                } finally {
                    synchronized(this@Coalescer) { running = null }
                }
            }
            running = flight
            flight to true
        }
        if (start.second) start.first.start()
        return start.first.await()
    }
}
