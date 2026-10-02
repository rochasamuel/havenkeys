package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ExpiryTextTest {
    @Test
    fun aCombinedExpiryInAnyCommonShape() {
        for (typed in listOf("04/33", "04 / 33", "0433", "4/33", "04/2033", "04-2033", "2033-04-01")) {
            assertEquals(typed, "2033-04", ExpiryText.of(typed, null, null))
        }
    }

    @Test
    fun aMonthAndAYearFromTextOrLists() {
        assertEquals("2033-04", ExpiryText.of(null, "04", "2033"))
        assertEquals("2033-04", ExpiryText.of(null, "04 - Abril", "33"))
        assertEquals("2033-04", ExpiryText.of(null, "April", "2033"))
    }

    @Test
    fun anythingElseIsNoExpiry() {
        assertNull(ExpiryText.of("13/33", null, null))
        assertNull(ExpiryText.of("Month", null, null))
        assertNull(ExpiryText.of(null, "04", null))
        assertNull(ExpiryText.of(null, "04", "123"))
    }
}
