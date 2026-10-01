package net.havenkeys.android.security

import android.content.Context
import android.provider.Settings

object BootCount {
    /** -1 when unknown; Rust refuses to enroll or unlock with an unknown count. */
    fun current(context: Context): Long =
        Settings.Global.getInt(context.contentResolver, Settings.Global.BOOT_COUNT, -1).toLong()
}
