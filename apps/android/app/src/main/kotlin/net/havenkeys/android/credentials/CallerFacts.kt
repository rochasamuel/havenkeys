package net.havenkeys.android.credentials

import android.os.Build
import androidx.annotation.RequiresApi
import androidx.credentials.provider.CallingAppInfo
import java.security.MessageDigest
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.privilegedBrowsersJson

/**
 * The caller as Android reports it. The origin is read only through
 * `getOrigin` with Rust's allowlist, and Rust checks the caller against the
 * same list again; an origin Android will not vouch for is dropped, which
 * makes the caller an app.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal fun CallingAppInfo.toCaller(): CredentialCaller {
    val certs = signingInfo.apkContentsSigners.orEmpty().map {
        MessageDigest.getInstance("SHA-256").digest(it.toByteArray())
    }
    return CredentialCaller(packageName, certs, verifiedOrigin())
}

@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
@Suppress("SwallowedException")
private fun CallingAppInfo.verifiedOrigin(): String? = if (!isOriginPopulated()) {
    null
} else {
    try {
        getOrigin(privilegedBrowsersJson())
    } catch (e: IllegalArgumentException) {
        null
    } catch (e: IllegalStateException) {
        null
    }
}
