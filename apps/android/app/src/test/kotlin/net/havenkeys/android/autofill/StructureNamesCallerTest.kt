package net.havenkeys.android.autofill

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class StructureNamesCallerTest {
    @Test
    fun theSameAppCounts() = assertTrue(structureNamesCaller("com.github.android", "com.github.android"))

    @Test
    fun anotherAppIsRefused() = assertFalse(structureNamesCaller("com.android.chrome", "com.evil.app"))

    @Test
    fun anUnknownCallerIsRefused() = assertFalse(structureNamesCaller("com.github.android", null))

    @Test
    fun aStructureNamingNoAppIsRefused() {
        assertFalse(structureNamesCaller("", ""))
        assertFalse(structureNamesCaller("", "com.github.android"))
    }
}
