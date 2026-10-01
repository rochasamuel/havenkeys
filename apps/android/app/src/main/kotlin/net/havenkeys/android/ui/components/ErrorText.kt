package net.havenkeys.android.ui.components

import androidx.annotation.StringRes
import net.havenkeys.android.R

private val texts = mapOf(
    "unlock_failed" to R.string.error_unlock_failed,
    "bundle_refused" to R.string.error_bundle_refused,
    "locked" to R.string.error_locked,
    "offline" to R.string.error_offline,
    "sign_in_failed" to R.string.error_sign_in_failed,
    "invalid_kit" to R.string.error_invalid_kit,
    "secret_key_required" to R.string.error_secret_key_required,
    "rate_limited" to R.string.error_rate_limited,
    "invalid_server_url" to R.string.error_invalid_server_url,
    "denied" to R.string.error_denied,
    "not_found" to R.string.error_not_found,
    "keychain_unavailable" to R.string.error_keystore,
    "biometric_unavailable" to R.string.error_biometric_unavailable,
)

/** The text for a `MobileException.Failed` code. Rust's own detail is never shown. */
@StringRes
fun errorText(code: String): Int = texts[code] ?: R.string.error_internal
