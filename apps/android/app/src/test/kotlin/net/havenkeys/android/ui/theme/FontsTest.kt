package net.havenkeys.android.ui.theme

import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontListFontFamily
import androidx.compose.ui.text.font.FontStyle
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FontsTest {
    private fun FontFamily.weights(style: FontStyle = FontStyle.Normal): List<Int> =
        (this as FontListFontFamily).fonts.filter { it.style == style }.map { it.weight.weight }

    @Test
    fun exactlyTheFourBuiltFilesAreBundledAndTheyStaySmall() {
        val files = File("src/main/res/font").listFiles().orEmpty().associate { it.name to it.length() }
        assertEquals(
            setOf("hanken_grotesk.ttf", "havenkeys_serif.ttf", "havenkeys_serif_italic.ttf", "jetbrains_mono.ttf"),
            files.keys,
        )
        assertTrue("fonts add ${files.values.sum()} bytes", files.values.sum() < 500_000)
    }

    @Test
    fun theFamiliesDeclareTheWeightsTheDesktopUses() {
        assertEquals(listOf(400, 500, 550, 560, 600, 640, 650, 660), HankenGrotesk.weights())
        assertEquals(listOf(400, 460, 470, 480, 500, 520), HavenSerif.weights())
        assertEquals(listOf(400), HavenSerif.weights(FontStyle.Italic))
        assertEquals(listOf(500), JetBrainsMono.weights())
    }
}
