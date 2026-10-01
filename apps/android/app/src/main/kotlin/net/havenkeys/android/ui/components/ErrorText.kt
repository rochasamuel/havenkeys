package net.havenkeys.android.ui.components

import androidx.annotation.StringRes
import net.havenkeys.android.R

/** The text for a `MobileException.Failed` code. Rust's own detail is never shown. */
@StringRes
fun errorText(code: String): Int = when (code) {
    "unlock_failed" -> R.string.error_unlock_failed
    "bundle_refused" -> R.string.error_bundle_refused
    "locked" -> R.string.error_locked
    "offline" -> R.string.error_offline
    "sign_in_failed" -> R.string.error_sign_in_failed
    "invalid_kit" -> R.string.error_invalid_kit
    "secret_key_required" -> R.string.error_secret_key_required
    "rate_limited" -> R.string.error_rate_limited
    "invalid_server_url" -> R.string.error_invalid_server_url
    "denied" -> R.string.error_denied
    "not_found" -> R.string.error_not_found
    "keychain_unavailable" -> R.string.error_keystore
    else -> R.string.error_internal
}
