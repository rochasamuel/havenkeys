package net.havenkeys.android.ui.kit

import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class HavenIconTest {
    @get:Rule
    val rule = createComposeRule()

    private val desktop: Map<String, String> = Regex("""^\s+(\w+): "([^"]+)",$""", RegexOption.MULTILINE)
        .findAll(File("../../desktop/src/components/Icon.tsx").readText())
        .associate { it.groupValues[1] to it.groupValues[2] }

    @Test
    fun theSetIsTheDesktopsPathForPath() {
        assertEquals(desktop.keys, HavenIcon.entries.map { it.desktopName }.toSet())
        HavenIcon.entries.forEach { assertEquals(it.name, desktop.getValue(it.desktopName), it.path) }
    }

    @Test
    fun everyPathParses() {
        HavenIcon.entries.forEach { assertTrue(it.name, PathParser().parsePathString(it.path).toNodes().isNotEmpty()) }
    }

    @Test
    fun aNamedGlyphIsAnImageAndAnUnnamedOneIsDecoration() {
        rule.setKit {
            IconGlyph(HavenIcon.Lock, contentDescription = "Locked")
            IconGlyph(HavenIcon.Copy, contentDescription = null)
        }
        rule.onNodeWithContentDescription("Locked").assert(hasRole(Role.Image))
        rule.onAllNodes(hasRole(Role.Image)).assertCountEquals(1)
    }
}
