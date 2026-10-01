package net.havenkeys.android

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
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
    fun everyActivitySetsFlagSecure() {
        val activity = Regex("""class\s+\w+\s*(\([^)]*\))?\s*:\s*(FragmentActivity|ComponentActivity|Activity)\(\)""")
        val sources = File("src/main/kotlin").walk().filter { it.extension == "kt" }.toList()
        val activities = sources.filter { activity.containsMatchIn(it.readText()) }
        assertTrue("found ${activities.size} activities", activities.size >= 3)
        for (file in activities) {
            assertTrue("${file.name} must set FLAG_SECURE", file.readText().contains("FLAG_SECURE"))
        }
    }
}
