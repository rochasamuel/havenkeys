package net.havenkeys.android.credentials

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class OneAtATimeTest {
    /** Final review (stage 4): a double tap on Save launched two verifications (two prompts, two creates). */
    @Test
    fun aSecondTapWhileTheFirstRunsStartsNothing() = runTest(UnconfinedTestDispatcher()) {
        val guard = OneAtATime()
        val gate = CompletableDeferred<Unit>()
        var runs = 0
        assertTrue(guard.launch(this) { runs++; gate.await() })
        assertFalse(guard.launch(this) { runs++ })
        assertEquals(1, runs)
        gate.complete(Unit)
        assertFalse(guard.running)
        assertTrue(guard.launch(this) { runs++ })
        assertEquals(2, runs)
    }

    @Test
    fun aFailedRunLetsTheNextTapThrough() = runTest(UnconfinedTestDispatcher()) {
        val guard = OneAtATime()
        val failing = CoroutineScope(coroutineContext + SupervisorJob() + CoroutineExceptionHandler { _, _ -> })
        guard.launch(failing) { error("cancelled prompt") }
        assertFalse(guard.running)
    }
}
