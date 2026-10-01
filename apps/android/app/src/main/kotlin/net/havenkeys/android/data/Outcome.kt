package net.havenkeys.android.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.havenkeys_mobile.MobileException

sealed interface Outcome<out T> {
    data class Ok<T>(val value: T) : Outcome<T> {
        // The value may be a secret (a revealed password, a TOTP code).
        override fun toString() = "Ok(…)"
    }

    /** Only the stable code crosses into the UI, which localizes it. */
    data class Failed(val code: String) : Outcome<Nothing>
}

/** Every Rust call goes through here: off the main thread, no exceptions out. */
suspend fun <T> rust(block: () -> T): Outcome<T> = withContext(Dispatchers.IO) {
    try {
        Outcome.Ok(block())
    } catch (e: MobileException.Failed) {
        Outcome.Failed(e.code)
    } catch (@Suppress("TooGenericExceptionCaught", "SwallowedException") e: Exception) {
        // The message is never kept: it could quote anything.
        Outcome.Failed("internal")
    }
}
