package net.havenkeys.android.ui.unlock

import org.junit.Assert.assertEquals
import org.junit.Test

class EnrollmentTest {
    @Test
    fun aRefusedEnrollmentIsNotAWrongPassword() {
        assertEquals("biometric_unavailable", enrollmentErrorCode("bundle_refused"))
    }

    @Test
    fun aWrongPasswordStaysAWrongPassword() {
        assertEquals("unlock_failed", enrollmentErrorCode("unlock_failed"))
    }
}
