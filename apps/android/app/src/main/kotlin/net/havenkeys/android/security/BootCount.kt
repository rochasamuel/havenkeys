package net.havenkeys.android.security

import android.content.Context
import android.provider.Settings

object BootCount {
    /**
     * -1 when unknown. Rust only compares counts, so -1 would match -1: enrolling
     * must refuse a negative count or the bundle stops being bound to this boot.
     */
    fun current(context: Context): Long =
        Settings.Global.getInt(context.contentResolver, Settings.Global.BOOT_COUNT, -1).toLong()
}
