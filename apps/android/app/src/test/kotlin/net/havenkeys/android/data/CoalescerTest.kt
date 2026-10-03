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
        val coalescer = Coalescer<Unit>(backgroundScope)
        val gate = CompletableDeferred<Unit>()
        var runs = 0
        val job: suspend () -> Outcome<Unit> = {
            runs++
            gate.await()
            Outcome.Failed("offline")
        }
        val first = async { coalescer.run(job = job) }
        val second = async { coalescer.run(job = job) }
        gate.complete(Unit)
        assertEquals(Outcome.Failed("offline"), first.await())
        assertEquals(Outcome.Failed("offline"), second.await())
        assertEquals(1, runs)
    }

    @Test
    fun cancellingTheFirstCallerDoesNotCancelTheRunOrTheOthers() = runTest(UnconfinedTestDispatcher()) {
        val coalescer = Coalescer<Unit>(backgroundScope)
        val gate = CompletableDeferred<Unit>()
        var runs = 0
        val job: suspend () -> Outcome<Unit> = {
            runs++
            gate.await()
            Outcome.Failed("real")
        }
        val first = async { coalescer.run(job = job) }
        val second = async { coalescer.run(job = job) }
        first.cancel()
        gate.complete(Unit)
        assertEquals(Outcome.Failed("real"), second.await())
        assertEquals(1, runs)
        assertEquals(Outcome.Ok(Unit), coalescer.run { runs++; Outcome.Ok(Unit) })
        assertEquals(2, runs)
    }

    @Test
    fun aFreshCallWaitsForTheRunInFlightThenRunsAgain() = runTest(UnconfinedTestDispatcher()) {
        val coalescer = Coalescer<Unit>(backgroundScope)
        val gate = CompletableDeferred<Unit>()
        val order = mutableListOf<String>()
        val slow = async {
            coalescer.run { order += "start"; gate.await(); order += "end"; Outcome.Ok(Unit) }
        }
        val fresh = async { coalescer.run(fresh = true) { order += "fresh"; Outcome.Ok(Unit) } }
        assertEquals(listOf("start"), order)
        gate.complete(Unit)
        slow.await()
        fresh.await()
        assertEquals(listOf("start", "end", "fresh"), order)
    }
}
