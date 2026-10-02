package net.havenkeys.android.security

import android.app.Activity
import android.view.View
import android.view.WindowManager

/**
 * Every window that unlocks, confirms or shows vault data: no screenshots
 * or recents thumbnail; no autofill service may read or save what is typed
 * here (CLAUDE.md §9); and a tap that passed through another app's overlay
 * is not the user's (tapjacking).
 */
fun Activity.hardenWindow() {
    window.setFlags(WindowManager.LayoutParams.FLAG_SECURE, WindowManager.LayoutParams.FLAG_SECURE)
    window.decorView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
    window.decorView.filterTouchesWhenObscured = true
}
