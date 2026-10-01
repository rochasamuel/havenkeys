package net.havenkeys.android.ui.components

import net.havenkeys.android.R
import org.junit.Assert.assertEquals
import org.junit.Test

class ErrorTextTest {
    @Test
    fun everyCodeTheAppMatchesOnHasItsOwnText() {
        val known = listOf(
            "unlock_failed", "bundle_refused", "locked", "offline", "sign_in_failed",
            "invalid_kit", "secret_key_required", "rate_limited", "invalid_server_url",
            "denied", "not_found", "keychain_unavailable", "biometric_unavailable", "internal",
        )
        val texts = known.map(::errorText)
        assertEquals(known.size, texts.toSet().size)
    }

    @Test
    fun anUnknownCodeFallsBackToTheGenericText() {
        assertEquals(R.string.error_internal, errorText("something_new"))
    }
}
