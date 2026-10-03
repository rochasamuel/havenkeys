package net.havenkeys.android.ui.theme

import androidx.compose.ui.unit.sp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class HavenTypographyTest {
    private val t = HavenTypography
    private val serif = listOf(t.display, t.headline, t.title, t.titleSmall, t.monogram)
    private val mono = listOf(t.secret, t.code)
    private val sans = listOf(
        t.body,
        t.value,
        t.rowTitle,
        t.rowSubtitle,
        t.label,
        t.groupTitle,
        t.button,
        t.pill,
        t.masked,
    )

    @Test
    fun theSpecsPhoneSizes() {
        assertEquals(15.sp, t.body.fontSize)
        assertEquals(16.sp, t.rowTitle.fontSize)
        assertEquals(13.sp, t.label.fontSize)
        assertEquals(28.sp, t.headline.fontSize)
        assertEquals(22.sp, t.code.fontSize)
    }

    @Test
    fun eachFaceKeepsItsJob() {
        serif.forEach { assertSame(HavenSerif, it.fontFamily) }
        mono.forEach { assertSame(JetBrainsMono, it.fontFamily) }
        sans.forEach { assertSame(HankenGrotesk, it.fontFamily) }
    }

    @Test
    fun everyWeightIsOneTheFileWasCutFor() {
        serif.forEach { assertTrue("${it.fontWeight}", it.fontWeight!!.weight in SerifWeights) }
        mono.forEach { assertEquals(500, it.fontWeight!!.weight) }
        sans.forEach { assertTrue("${it.fontWeight}", it.fontWeight!!.weight in HankenWeights) }
    }

    @Test
    fun codesAreTabularSoTheyDoNotJitter() = assertTrue(t.code.fontFeatureSettings!!.contains("tnum"))
}
