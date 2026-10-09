package net.havenkeys.android.ui.components

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import androidx.core.net.toUri

/** Opens a link exactly as given; a phone with no browser simply does nothing. */
fun openLink(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
    } catch (@Suppress("SwallowedException") e: ActivityNotFoundException) {
        // Nothing to open it with.
    }
}
