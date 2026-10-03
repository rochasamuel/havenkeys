package net.havenkeys.android.autofill

import java.io.File
import net.havenkeys.android.ui.kit.HavenIcon
import org.junit.Assert.assertEquals
import org.junit.Test

class CopyRowIconTest {
    @Test
    fun theCopyRowDrawsTheDesktopsCopyGlyph() {
        val xml = File("src/main/res/drawable/ic_autofill_copy.xml").readText()
        val path = Regex("android:pathData=\"([^\"]+)\"").find(xml)?.groupValues?.get(1)
        assertEquals(HavenIcon.Copy.path, path)
    }
}
