package net.havenkeys.android.autofill

import android.view.inputmethod.InlineSuggestionsRequest

/**
 * The keyboard's last request for chips, as the last fill request carried it.
 * Android attaches it to a tapped row's Intent only when that row came from
 * the session's latest fill request; without it an unlock's answer had no
 * chips and, replacing the cached response, left the keyboard empty until the
 * page reloaded. Holds no secret: the keyboard's styles and chip count.
 */
internal object KeyboardRequests {
    @Volatile private var last: InlineSuggestionsRequest? = null

    fun remember(request: InlineSuggestionsRequest?) {
        if (request != null) last = request
    }

    fun orLast(attached: InlineSuggestionsRequest?): InlineSuggestionsRequest? = attached ?: last
}
