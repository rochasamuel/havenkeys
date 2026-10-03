package net.havenkeys.android.catalogue

import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToIndex
import androidx.compose.ui.test.performScrollToNode
import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.w3c.dom.Element

@RunWith(RobolectricTestRunner::class)
class CatalogueTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun everyKitComponentHasItsPlace() {
        val helpers = setOf("HavenText", "HavenPress", "KitPreview", "NoPersonalizedLearning", "HavenClickable")
        val kit = File("src/main/kotlin/net/havenkeys/android/ui/kit").listFiles().orEmpty()
            .map { it.nameWithoutExtension }.toSet() - helpers
        val shown = catalogueSections().flatMap { it.components }.toSet()
        assertEquals(kit.sorted(), shown.sorted())
    }

    @Test
    fun everySectionRendersInBothThemes() {
        rule.setContent { KitCatalogue() }
        for (theme in listOf("Light", "Dark")) {
            rule.onNodeWithTag(CATALOGUE_LIST).performScrollToIndex(0)
            rule.onNodeWithText(theme).performClick()
            rule.waitForIdle()
            rule.onNodeWithTag(CATALOGUE_LIST).assert(SemanticsMatcher.expectValue(CatalogueDark, theme == "Dark"))
            catalogueSections().forEach { section ->
                val heading = hasText(section.title) and isHeading()
                rule.onNodeWithTag(CATALOGUE_LIST).performScrollToNode(heading)
                rule.onNode(heading).assertIsDisplayed()
            }
        }
    }

    @Test
    fun onlyDebugBuildsDeclareTheCatalogue() {
        val ns = "http://schemas.android.com/apk/res/android"
        val debug = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }.newDocumentBuilder()
            .parse(File("src/debug/AndroidManifest.xml")).documentElement
        val activities = debug.getElementsByTagName("activity")
        assertEquals(1, activities.length)
        assertEquals(".catalogue.KitCatalogueActivity", (activities.item(0) as Element).getAttributeNS(ns, "name"))
        assertFalse(File("src/main/AndroidManifest.xml").readText().contains("catalogue"))
    }
}
