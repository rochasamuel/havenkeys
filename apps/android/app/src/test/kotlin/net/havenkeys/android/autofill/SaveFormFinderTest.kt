package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class SaveFormFinderTest {
    private fun find(vararg fields: FieldFacts): SaveForm? {
        val list = fields.toList()
        return SaveFormFinder.find(list, LoginFormFinder.find(list))
    }

    private val newPw = mapOf("type" to "password", "autocomplete" to "new-password")
    private val currentPw = mapOf("type" to "password", "autocomplete" to "current-password")

    @Test
    fun aLoginSavesItsUsernameAndPassword() {
        val user = field(inputType = email)
        val pw = field(inputType = password)
        val save = find(user, pw)!!
        assertEquals(user.index, save.username)
        assertEquals(pw.index, save.password)
        assertNull(save.current)
    }

    @Test
    fun aSignUpSavesTheNewPassword() {
        val user = field(inputType = email)
        val first = field(html = newPw)
        val again = field(html = newPw)
        val save = find(user, first, again)!!
        assertEquals(user.index, save.username)
        assertEquals(first.index, save.password)
        assertNull(save.current)
    }

    @Test
    fun aChangePasswordFormSavesTheNewOneWithTheCurrent() {
        val current = field(html = currentPw)
        val new = field(html = newPw)
        val again = field(html = newPw)
        val save = find(current, new, again)!!
        assertEquals(new.index, save.password)
        assertEquals(current.index, save.current)
    }

    @Test
    fun aLoneNewPasswordBesideAUsernameIsTheLoginPassword() {
        val user = field(inputType = email)
        val pw = field(hints = listOf("new-password"), inputType = password)
        val save = find(user, pw)!!
        assertEquals(pw.index, save.password)
        assertNull(save.current)
    }

    @Test
    fun aUsernameStepIsKeptForTheNextStep() {
        val user = field(inputType = email)
        val save = find(field(), user)!!
        assertEquals(user.index, save.username)
        assertNull(save.password)
    }

    @Test
    fun aCodeStepHasNothingToSave() {
        assertNull(find(field(html = mapOf("autocomplete" to "one-time-code"))))
    }

    @Test
    fun aNewPasswordInAnotherFrameIsNotSavedForThisOne() {
        val user = field(inputType = email, domain = "example.com", scheme = "https")
        val pw = field(inputType = password, domain = "example.com", scheme = "https", focused = true)
        val other = field(html = newPw, domain = "ads.example.net", scheme = "https")
        val save = find(user, pw, other)!!
        assertEquals(pw.index, save.password)
        assertEquals("example.com", save.webDomain)
    }
}
