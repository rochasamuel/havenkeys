package net.havenkeys.android.data

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class CoalescerTest {
    @Test
    fun concurrentCallersShareOneRun() = runTest(UnconfinedTestDispatcher()) {
        val coalescer = Coalescer<Unit>()
        val gate = CompletableDeferred<Unit>()
        var runs = 0
        val job: suspend () -> Outcome<Unit> = {
            runs++
            gate.await()
            Outcome.Failed("offline")
        }
        val first = async { coalescer.run(job) }
        val second = async { coalescer.run(job) }
        gate.complete(Unit)
        assertEquals(Outcome.Failed("offline"), first.await())
        assertEquals(Outcome.Failed("offline"), second.await())
        assertEquals(1, runs)
    }

    @Test
    fun aLaterCallRunsAgain() = runTest(UnconfinedTestDispatcher()) {
        val coalescer = Coalescer<Unit>()
        var runs = 0
        repeat(2) { coalescer.run { runs++; Outcome.Ok(Unit) } }
        assertEquals(2, runs)
    }
}
