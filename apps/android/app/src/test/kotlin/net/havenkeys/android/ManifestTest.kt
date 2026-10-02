package net.havenkeys.android

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element

/** The hardening the spec asks of the manifest (§9.4) and of every activity. */
class ManifestTest {
    private val ns = "http://schemas.android.com/apk/res/android"
    private val manifest = DocumentBuilderFactory.newInstance()
        .apply { isNamespaceAware = true }
        .newDocumentBuilder()
        .parse(File("src/main/AndroidManifest.xml"))
        .documentElement

    private fun elements(tag: String): List<Element> {
        val nodes = manifest.getElementsByTagName(tag)
        return (0 until nodes.length).map { nodes.item(it) as Element }
    }

    private fun Element.android(name: String) = getAttributeNS(ns, name)

    @Test
    fun onlyTheLauncherActivityIsExported() {
        val activities = elements("activity")
        assertTrue(activities.any { it.android("name") == ".autofill.AutofillAuthActivity" })
        assertTrue(activities.any { it.android("name") == ".autofill.AutofillSearchActivity" })
        for (activity in activities) {
            val name = activity.android("name")
            if (name == ".MainActivity") {
                assertEquals("true", activity.android("exported"))
            } else {
                assertEquals("$name must not be exported", "false", activity.android("exported"))
            }
        }
    }

    @Test
    fun theOnlyServiceIsTheAutofillServiceGuardedByTheSystemPermission() {
        val service = elements("service").single()
        assertEquals(".autofill.HavenAutofillService", service.android("name"))
        assertEquals("android.permission.BIND_AUTOFILL_SERVICE", service.android("permission"))
    }

    @Test
    fun noProviderOrReceiver() {
        assertTrue(elements("provider").isEmpty())
        assertTrue(elements("receiver").isEmpty())
    }

    @Test
    fun backupIsOff() {
        assertEquals("false", elements("application").single().android("allowBackup"))
    }

    @Test
    fun permissionsAreExactlyTheM1Set() {
        val permissions = elements("uses-permission").map { it.android("name") }.toSet()
        assertEquals(
            setOf("android.permission.INTERNET", "android.permission.USE_BIOMETRIC", "android.permission.CAMERA"),
            permissions,
        )
    }

    @Test
    fun onlyTheGithubFlavorMaySeeOtherApps() {
        // Autofill must read the signing certificate of the app or browser
        // it fills; Android 11+ hides other packages without this.
        val github = DocumentBuilderFactory.newInstance()
            .apply { isNamespaceAware = true }
            .newDocumentBuilder()
            .parse(File("src/github/AndroidManifest.xml"))
            .documentElement
        val nodes = github.getElementsByTagName("uses-permission")
        val added = (0 until nodes.length).map { (nodes.item(it) as Element).android("name") }.toSet()
        assertEquals(setOf("android.permission.QUERY_ALL_PACKAGES"), added)
        assertFalse(File("src/play/AndroidManifest.xml").exists())
    }

    private val sources = File("src/main/kotlin").walk().filter { it.extension == "kt" }.toList()

    private val activities: List<File> by lazy {
        val activity = Regex("""class\s+\w+\s*(\([^)]*\))?\s*:\s*(FragmentActivity|ComponentActivity|Activity)\(\)""")
        sources.filter { activity.containsMatchIn(it.readText()) }.also {
            assertTrue("found ${it.size} activities", it.size >= 3)
        }
    }

    @Test
    fun everyActivitySetsFlagSecure() {
        for (file in activities) {
            assertTrue("${file.name} must set FLAG_SECURE", file.readText().contains("FLAG_SECURE"))
        }
    }

    /** The master password and the Secret Key never reach a third-party autofill service (CLAUDE.md §9). */
    @Test
    fun everyActivityKeepsItsFieldsFromAutofill() {
        for (file in activities) {
            assertTrue(
                "${file.name} must exclude its window from autofill",
                file.readText().contains("importantForAutofill = View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS"),
            )
        }
        for (file in sources.filter { "AlertDialog(" in it.readText() && "OutlinedTextField(" in it.readText() }) {
            assertTrue(
                "${file.name}: a dialog with a text field needs SecureDialogWindow",
                "SecureDialogWindow(" in file.readText(),
            )
        }
        for (file in sources) {
            assertFalse(
                "${file.name} must not hint a password to autofill",
                Regex("""ContentType\.(Password|NewPassword)""").containsMatchIn(file.readText()),
            )
        }
    }

    @Test
    fun theAutofillActivitiesIgnoreObscuredTaps() {
        for (name in listOf("AutofillAuthActivity.kt", "AutofillSearchActivity.kt")) {
            val text = activities.single { it.name == name }.readText()
            assertTrue("$name must filter obscured touches", "filterTouchesWhenObscured = true" in text)
        }
        val search = activities.single { it.name == "AutofillSearchActivity.kt" }.readText()
        assertTrue(
            "the binding dialog must filter obscured touches",
            "SecureDialogWindow(ignoreObscuredTouches = true)" in search,
        )
    }
}
