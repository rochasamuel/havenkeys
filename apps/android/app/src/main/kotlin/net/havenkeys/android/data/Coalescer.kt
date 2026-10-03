package net.havenkeys.android.data

import kotlinx.coroutines.CompletableDeferred

/**
 * Runs one job at a time: a caller that arrives while it runs waits for that
 * run's outcome instead of starting another. Sync now and pull to refresh
 * share one in-flight sync this way.
 */
class Coalescer<T> {
    private var running: CompletableDeferred<Outcome<T>>? = null

    suspend fun run(job: suspend () -> Outcome<T>): Outcome<T> {
        val (flight, owner) = synchronized(this) {
            val current = running
            if (current != null) {
                current to false
            } else {
                CompletableDeferred<Outcome<T>>().also { running = it } to true
            }
        }
        if (!owner) return flight.await()
        var result: Outcome<T> = Outcome.Failed("cancelled")
        try {
            result = job()
            return result
        } finally {
            synchronized(this) { running = null }
            flight.complete(result)
        }
    }
}
