package net.havenkeys.android.autofill

import android.content.IntentSender
import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.view.View
import android.view.autofill.AutofillValue
import android.widget.FrameLayout
import android.widget.RemoteViews
import android.widget.TextView
import net.havenkeys.android.R
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
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

    // These getters are hidden from the SDK but present at runtime.
    @Suppress("UNCHECKED_CAST", "SpreadOperator")
    private fun <T> Any.hidden(name: String, vararg args: Int): T? {
        val method = javaClass.getMethod(name, *Array(args.size) { Int::class.javaPrimitiveType })
        return method.invoke(this, *args.toTypedArray()) as T?
    }

    private fun FillResponse.datasets(): List<Dataset> = hidden("getDatasets") ?: emptyList()
    private fun Dataset.texts(): List<String> {
        val view = requireNotNull(hidden<RemoteViews>("getFieldPresentation", 0)).apply(context, FrameLayout(context))
        return listOf(R.id.autofill_title, R.id.autofill_subtitle)
            .map { view.findViewById<TextView>(it).text.toString() }
    }

    @Test
    fun aPageWithNoLoginGetsAnEmptyResponseAfterUnlock() {
        val response = factory.response(FillPlan.Offer(emptyList(), search = false)).orEmptyAfterUnlock()
        assertEquals(emptyList<Dataset>(), response.datasets())
        assertEquals(emptyList<Dataset>(), factory.response(FillPlan.Nothing).orEmptyAfterUnlock().datasets())
        // Cards and the Identity answer their unlock the same way.
        assertEquals(emptyList<Dataset>(), null.orEmptyAfterUnlock().datasets())
    }

    @Test
    fun afterAnUnlockThatFoundNothingOneRowOpensHavenKeys() {
        val plan = FillPlan.Offer(emptyList(), search = false, openApp = true)
        val row = factory.response(plan).orEmptyAfterUnlock().datasets().single()
        assertEquals(listOf("No logins found", "Open HavenKeys"), row.texts())
        assertNotNull(row.hidden<IntentSender>("getAuthentication"))
        // It fills nothing.
        assertTrue(row.hidden<List<AutofillValue?>>("getFieldValues").orEmpty().all { it == null })
    }
}
