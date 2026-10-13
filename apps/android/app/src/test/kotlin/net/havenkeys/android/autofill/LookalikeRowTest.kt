package net.havenkeys.android.autofill

import android.content.IntentSender
import android.service.autofill.Dataset
import android.service.autofill.FillResponse
import android.service.autofill.SaveInfo
import android.view.View
import android.view.autofill.AutofillId
import android.view.autofill.AutofillValue
import android.widget.FrameLayout
import android.widget.RemoteViews
import android.widget.TextView
import java.io.File
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows
import org.robolectric.fakes.RoboIntentSender

@RunWith(RobolectricTestRunner::class)
class LookalikeRowTest {
    private val context = RuntimeEnvironment.getApplication()
    private val ids = List(2) { View(context).autofillId }
    private val screen = ParsedScreen("com.android.chrome", emptyList(), ids, "xn--gthub-n2e.com", "https")
    private val login = LoginForm(listOf(0), listOf(1), emptyList(), "xn--gthub-n2e.com", "https", 0)
    private val save = SaveForm(0, 1, null, "xn--gthub-n2e.com", "https")

    // These getters are hidden from the SDK but present at runtime.
    @Suppress("UNCHECKED_CAST", "SpreadOperator")
    private fun <T> Any.hidden(name: String, vararg args: Int): T? {
        val method = javaClass.getMethod(name, *args.map { Int::class.javaPrimitiveType }.toTypedArray())
        return method.invoke(this, *args.toTypedArray()) as T?
    }

    private fun SaveInfo.description(): CharSequence? = hidden("getDescription")
    private fun FillResponse.datasets(): List<Dataset> = hidden("getDatasets") ?: emptyList()
    private fun FillResponse.saveInfo(): SaveInfo? = hidden("getSaveInfo")
    private fun Dataset.fieldIds(): List<AutofillId> = hidden("getFieldIds") ?: emptyList()
    private fun Dataset.fieldValues(): List<AutofillValue?> = hidden("getFieldValues") ?: emptyList()
    private fun Dataset.authentication(): IntentSender? = hidden("getAuthentication")

    private fun response(lookalikeOf: String?, saveable: Boolean = false) =
        DatasetFactory(context, screen, login, save, null)
            .response(FillPlan.Offer(emptyList(), search = false, save = saveable, lookalikeOf = lookalikeOf))

    private fun Dataset.texts(): List<String> {
        val view = requireNotNull(hidden<RemoteViews>("getFieldPresentation", 0)).apply(context, FrameLayout(context))
        return listOf(R.id.autofill_title, R.id.autofill_subtitle)
            .map { view.findViewById<TextView>(it).text.toString() }
    }

    @Test
    fun theWarningNamesTheSavedSiteOnly() {
        val title = context.getString(R.string.autofill_lookalike_title, "github.com")
        assertTrue(title.contains("github.com"))
        assertTrue(context.lookalikeSaveDescription("github.com").contains("github.com"))
    }

    @Test
    fun aLookAlikePageGetsOneWarningRowThatFillsNothing() {
        val row = requireNotNull(response("github.com")).datasets().single()
        assertEquals(ids, row.fieldIds())
        assertTrue("the warning row carries no value", row.fieldValues().all { it == null })
        assertEquals(
            listOf(
                context.getString(R.string.autofill_lookalike_title, "github.com"),
                context.getString(R.string.autofill_lookalike_subtitle),
            ),
            row.texts(),
        )
        val intent = Shadows.shadowOf((row.authentication() as RoboIntentSender).pendingIntent).savedIntent
        assertEquals(DatasetFactory.MODE_LOOKALIKE, intent.getStringExtra(DatasetFactory.EXTRA_MODE))
        assertEquals("the intent carries only the mode", setOf(DatasetFactory.EXTRA_MODE), intent.extras?.keySet())
    }

    @Test
    fun noLookAlikeMeansNoWarningRow() {
        assertNull(response(null))
        assertTrue(requireNotNull(response(null, saveable = true)).datasets().isEmpty())
    }

    @Test
    fun theSaveSheetOfALookAlikePageSaysSo() {
        val warned = requireNotNull(response("github.com", saveable = true)).saveInfo()
        assertEquals(context.lookalikeSaveDescription("github.com"), warned?.description()?.toString())
        val plain = requireNotNull(response(null, saveable = true)).saveInfo()
        assertNotNull(plain)
        assertNull(plain?.description())
    }

    @Test
    fun theSaveSheetCarriesTheDescriptionOnlyWhenGiven() {
        val warned = requireNotNull(save.saveInfoIn(screen, context.lookalikeSaveDescription("github.com")))
        assertEquals(context.lookalikeSaveDescription("github.com"), warned.description()?.toString())
        assertNull(requireNotNull(save.saveInfoIn(screen)).description())
        val usernameFirst = SaveForm(username = 0, password = null, current = null, webDomain = null, webScheme = null)
        assertEquals("site", usernameFirst.saveInfoIn(screen, "site")?.description()?.toString())
    }

    @Test
    fun theWarningRowDrawsTheDesktopsAlertGlyph() {
        val xml = File("src/main/res/drawable/ic_autofill_warning.xml").readText()
        val path = Regex("android:pathData=\"([^\"]+)\"").find(xml)?.groupValues?.get(1)
        assertEquals(HavenIcon.Alert.path, path)
    }
}
