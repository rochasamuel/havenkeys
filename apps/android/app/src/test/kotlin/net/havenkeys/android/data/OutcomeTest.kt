package net.havenkeys.android.data

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.havenkeys_mobile.MobileException

class OutcomeTest {
    @Test
    fun aRustRefusalBecomesItsCode() = runTest {
        val r = rust<Int> { throw MobileException.Failed("locked", "The vault is locked.") }
        assertEquals(Outcome.Failed("locked"), r)
    }

    @Test
    fun anythingElseIsInternalAndCarriesNoMessage() = runTest {
        val r = rust<Int> { throw IllegalStateException("hunter2") }
        assertEquals(Outcome.Failed("internal"), r)
    }

    @Test
    fun aValuePassesThrough() = runTest {
        assertEquals(Outcome.Ok(3), rust { 3 })
    }
}
