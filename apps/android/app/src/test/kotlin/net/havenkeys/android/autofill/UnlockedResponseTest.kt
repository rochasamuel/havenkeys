package net.havenkeys.android.autofill

import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.view.View
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

/**
 * The answer to a tapped "Unlock HavenKeys" row replaces the locked
 * response Android cached for the screen. With nothing to offer it must
 * still be a response: a cancelled unlock leaves the unlock row in place,
 * so it came back on every tap of a page with no saved login.
 */
@RunWith(RobolectricTestRunner::class)
class UnlockedResponseTest {
    private val context = RuntimeEnvironment.getApplication()
    private val ids = List(2) { View(context).autofillId }
    private val screen = ParsedScreen("com.android.chrome", emptyList(), ids, "example.com", "https")
    private val login = LoginForm(listOf(0), listOf(1), emptyList(), "example.com", "https", 0)
    private val factory = DatasetFactory(context, screen, login, null, null)

    @Suppress("UNCHECKED_CAST")
    private fun FillResponse.datasets(): List<Dataset> =
        (javaClass.getMethod("getDatasets").invoke(this) as List<Dataset>?).orEmpty()

    @Test
    fun aPageWithNoLoginGetsAnEmptyResponseAfterUnlock() {
        val response = factory.response(FillPlan.Offer(emptyList(), search = false)).orEmptyAfterUnlock()
        assertEquals(emptyList<Dataset>(), response.datasets())
        assertEquals(emptyList<Dataset>(), factory.response(FillPlan.Nothing).orEmptyAfterUnlock().datasets())
        // Cards and the Identity answer their unlock the same way.
        assertEquals(emptyList<Dataset>(), null.orEmptyAfterUnlock().datasets())
    }
}
