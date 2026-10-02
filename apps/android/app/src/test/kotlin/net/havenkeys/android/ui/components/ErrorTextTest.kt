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

    @Test
    fun editingAndSavingCodesHaveTheirOwnText() {
        assertEquals(R.string.error_item_changed_elsewhere, errorText("item_changed_elsewhere"))
        assertEquals(R.string.error_title_required, errorText("title_required"))
        assertEquals(R.string.error_card_title_required, errorText("card_title_required"))
        assertEquals(R.string.error_invalid_totp, errorText("invalid_totp"))
        assertEquals(R.string.error_invalid_expiry, errorText("invalid_expiry"))
        assertEquals(R.string.error_invalid_website, errorText("invalid_website"))
        assertEquals(R.string.error_invalid_input, errorText("invalid_input"))
        assertEquals(R.string.error_invalid_input, errorText("invalid_field"))
    }

    @Test
    fun passkeyCodesHaveTheirOwnText() {
        assertEquals(R.string.passkey_exists, errorText("passkey_exists"))
        assertEquals(R.string.error_passkey_unsupported, errorText("unsupported_algorithm"))
    }
}
