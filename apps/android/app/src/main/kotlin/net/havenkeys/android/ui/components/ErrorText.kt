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
    "account_deleted" to R.string.error_account_deleted,
    "account_frozen" to R.string.error_account_frozen,
    "invalid_server_url" to R.string.error_invalid_server_url,
    "denied" to R.string.error_denied,
    "not_found" to R.string.error_not_found,
    "keychain_unavailable" to R.string.error_keystore,
    "biometric_unavailable" to R.string.error_biometric_unavailable,
    "item_changed_elsewhere" to R.string.error_item_changed_elsewhere,
    "title_required" to R.string.error_title_required,
    "card_title_required" to R.string.error_card_title_required,
    "pairing_gone" to R.string.error_pairing_gone,
    "pairing_other_server" to R.string.error_pairing_other_server,
    "pairing_failed" to R.string.error_pairing_failed,
    "invalid_totp" to R.string.error_invalid_totp,
    "invalid_expiry" to R.string.error_invalid_expiry,
    "invalid_website" to R.string.error_invalid_website,
    "invalid_input" to R.string.error_invalid_input,
    "passkey_exists" to R.string.passkey_exists,
    "unsupported_algorithm" to R.string.error_passkey_unsupported,
    // A field the editor should not have sent: the user can only retry.
    "invalid_field" to R.string.error_invalid_input,
)

/** The text for a `MobileException.Failed` code. Rust's own detail is never shown. */
@StringRes
fun errorText(code: String): Int = texts[code] ?: R.string.error_internal
