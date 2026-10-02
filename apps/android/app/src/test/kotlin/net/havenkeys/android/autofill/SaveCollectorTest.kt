package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.TargetFacts

class SaveCollectorTest {
    private fun site(domain: String) = TargetFacts("com.android.chrome", listOf(ByteArray(32)), domain, "https")

    @Test
    fun oneScreenGivesItsLogin() {
        val login = SaveCollector.collect(listOf(SubmittedForm(site("github.com"), "octo", "hunter2", null)))!!
        assertEquals("octo", login.username)
        assertEquals("hunter2", login.password)
    }

    @Test
    fun aUsernameFromTheStepBeforeIsUsedForTheSameSite() {
        val login = SaveCollector.collect(
            listOf(
                SubmittedForm(site("github.com"), "octo", null, null),
                SubmittedForm(site("github.com"), null, "hunter2", null),
            ),
        )!!
        assertEquals("octo", login.username)
    }

    @Test
    fun aUsernameFromAnotherSiteIsNot() {
        val login = SaveCollector.collect(
            listOf(
                SubmittedForm(site("evil.example"), "octo", null, null),
                SubmittedForm(site("github.com"), null, "hunter2", null),
            ),
        )!!
        assertNull(login.username)
    }

    @Test
    fun noPasswordNothingToSave() {
        assertNull(SaveCollector.collect(listOf(SubmittedForm(site("github.com"), "octo", null, null))))
        assertNull(SaveCollector.collect(listOf(SubmittedForm(site("github.com"), "octo", "", null))))
        assertNull(SaveCollector.collect(emptyList()))
    }

    @Test
    fun theCollectedLoginNamesNoValue() {
        val login = SaveCollector.collect(listOf(SubmittedForm(site("github.com"), "octo", "hunter2", null)))!!
        assertFalse(login.toString().contains("hunter2"))
        assertFalse(SubmittedForm(site("github.com"), "octo", "hunter2", null).toString().contains("hunter2"))
    }
}
