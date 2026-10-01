package net.havenkeys.android.ui.components

import android.view.View
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.ui.platform.LocalView

/**
 * A dialog has a window of its own, outside its activity's protections:
 * call this inside its content. Nothing in it is offered to an autofill
 * service (a master password is typed in one), and with
 * [ignoreObscuredTouches] a tap is dropped while another app's window
 * covers it.
 */
@Composable
fun SecureDialogWindow(ignoreObscuredTouches: Boolean = false) {
    val view = LocalView.current
    DisposableEffect(view) {
        view.rootView.importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        if (ignoreObscuredTouches) view.rootView.filterTouchesWhenObscured = true
        onDispose {}
    }
}
