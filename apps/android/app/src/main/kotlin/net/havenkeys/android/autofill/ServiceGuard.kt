package net.havenkeys.android.autofill

import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.CoroutineExceptionHandler

/**
 * The structure an autofill request carries is built by the app being
 * filled. Whatever it makes the parser or the planner throw ends that one
 * request with [fallback], never the process (security-review AN30). The
 * exception itself is dropped: it could quote anything.
 */
internal suspend fun <T> guarded(fallback: T, block: suspend () -> T): T = try {
    block()
} catch (e: CancellationException) {
    throw e
} catch (@Suppress("TooGenericExceptionCaught", "SwallowedException") e: Exception) {
    fallback
}

/** The last net for a coroutine of the service's scope: swallow rather than crash. */
internal val swallowUncaught = CoroutineExceptionHandler { _, _ -> }
