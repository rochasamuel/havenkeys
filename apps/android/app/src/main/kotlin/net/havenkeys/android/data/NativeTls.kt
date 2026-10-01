package net.havenkeys.android.data

import android.content.Context

/** Gives rustls-platform-verifier the app Context before the first connection. */
object NativeTls {
    init {
        System.loadLibrary("havenkeys_mobile")
    }

    @JvmStatic
    external fun init(context: Context)
}
