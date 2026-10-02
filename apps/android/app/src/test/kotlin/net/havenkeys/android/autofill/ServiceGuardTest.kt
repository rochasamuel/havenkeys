package net.havenkeys.android.autofill

import kotlin.coroutines.cancellation.CancellationException
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.fail
import org.junit.Test

class ServiceGuardTest {
    @Test
    fun aThrowingRequestAnswersTheFallback() = runTest {
        assertNull(guarded(null) { throw IllegalStateException("a hostile structure") })
        assertEquals("ok", guarded("fallback") { "ok" })
    }

    @Test
    fun cancellationIsNotSwallowed() = runTest {
        try {
            guarded(null) { throw CancellationException("cancelled") }
            fail("cancellation was swallowed")
        } catch (e: CancellationException) {
            assertEquals("cancelled", e.message)
        }
    }
}
