package net.havenkeys.android.autofill

import android.util.Size
import android.view.inputmethod.InlineSuggestionsRequest
import android.widget.inline.InlinePresentationSpec
import org.junit.Assert.assertSame
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Android attaches the keyboard's request to an unlock's Intent only when
 * the tapped row came from the session's latest fill request. Without it the
 * unlocked answer had no chips, and it replaced the cached response, so the
 * keyboard showed nothing until the page reloaded.
 */
@RunWith(RobolectricTestRunner::class)
class KeyboardRequestsTest {
    private fun keyboard(max: Int): InlineSuggestionsRequest = InlineSuggestionsRequest.Builder(
        listOf(InlinePresentationSpec.Builder(Size(0, 0), Size(100, 100)).build()),
    ).setMaxSuggestionCount(max).build()

    @Test
    fun anUnlockWithoutTheKeyboardsRequestUsesTheLastOneSeen() {
        val seen = keyboard(3)
        KeyboardRequests.remember(seen)
        assertSame(seen, KeyboardRequests.orLast(null))
    }

    @Test
    fun theAttachedRequestWinsAndARequestWithoutOneKeepsTheLast() {
        val seen = keyboard(3)
        KeyboardRequests.remember(seen)
        KeyboardRequests.remember(null)
        assertSame(seen, KeyboardRequests.orLast(null))
        val attached = keyboard(5)
        assertSame(attached, KeyboardRequests.orLast(attached))
    }
}
