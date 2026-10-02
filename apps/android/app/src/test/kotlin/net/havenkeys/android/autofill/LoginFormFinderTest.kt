package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.system.measureTimeMillis

class LoginFormFinderTest {
    private fun find(vararg fields: FieldFacts) = LoginFormFinder.find(fields.toList())

    @Test
    fun login() {
        val user = field(inputType = email)
        val pw = field(inputType = password, focused = true)
        val form = find(user, pw)!!
        assertEquals(listOf(user.index), form.usernames)
        assertEquals(listOf(pw.index), form.passwords)
        assertEquals(emptyList<Int>(), form.otps)
        assertEquals(pw.index, form.focused)
    }

    @Test
    fun emailFirst() {
        val user = field(inputType = email)
        val form = find(field(), user)!!
        assertEquals(listOf(user.index), form.usernames)
        assertEquals(emptyList<Int>(), form.passwords)
    }

    @Test
    fun passwordOnly() {
        val pw = field(inputType = password)
        val form = find(pw)!!
        assertEquals(emptyList<Int>(), form.usernames)
        assertEquals(listOf(pw.index), form.passwords)
    }

    @Test
    fun otpStep() {
        val otp = field(html = mapOf("autocomplete" to "one-time-code"))
        val form = find(otp)!!
        assertEquals(listOf(otp.index), form.otps)
        assertEquals(emptyList<Int>(), form.usernames + form.passwords)
    }

    @Test
    fun aFrameOfAnotherSiteIsLeftOut() {
        val user = field(inputType = email, domain = "accounts.example.com", scheme = "https")
        val pw = field(inputType = webPassword, domain = "evil.com", scheme = "https")
        val form = find(user, pw)!!
        assertEquals(emptyList<Int>(), form.usernames)
        assertEquals(listOf(pw.index), form.passwords)
        assertEquals("evil.com", form.webDomain)
        assertEquals("https", form.webScheme)
    }

    @Test
    fun theFocusedLoginFieldPicksTheFrame() {
        val user = field(inputType = email, domain = "accounts.example.com", scheme = "https", focused = true)
        val pw = field(inputType = webPassword, domain = "evil.com", scheme = "https")
        val form = find(user, pw)!!
        assertEquals(listOf(user.index), form.usernames)
        assertEquals(emptyList<Int>(), form.passwords)
        assertEquals("accounts.example.com", form.webDomain)
        assertEquals(user.index, form.focused)
    }

    @Test
    fun aFocusedFieldInAnotherFrameIsNotReportedAsFocused() {
        val search = field(id = "search", domain = "ads.example.net", scheme = "https", focused = true)
        val pw = field(inputType = webPassword, domain = "example.com", scheme = "https")
        val form = find(search, pw)!!
        assertEquals("example.com", form.webDomain)
        assertNull(form.focused)
    }

    @Test
    fun signUpIsNotFilled() {
        val user = field(inputType = email)
        val a = field(html = mapOf("type" to "password", "autocomplete" to "new-password"))
        val b = field(html = mapOf("type" to "password", "autocomplete" to "new-password"))
        assertNull(find(user, a, b))
        assertNull(find(a, b))
    }

    @Test
    fun aLoginWhosePasswordSaysNewPasswordIsStillALogin() {
        // dashboard.render.com/login as Chrome reported it: sites put
        // `new-password` on a login's only password to stop browsers filling.
        val user = field(
            hints = listOf("email"),
            inputType = 0,
            hint = "your@email.com",
            html = mapOf("name" to "email", "label" to "Email", "type" to "email"),
            domain = "dashboard.render.com",
            scheme = "https",
        )
        val pw = field(
            hints = listOf("new-password"),
            inputType = 0,
            hint = "correct horse battery staple",
            html = mapOf("name" to "password", "label" to "Password", "type" to "password"),
            focused = true,
            domain = "dashboard.render.com",
            scheme = "https",
        )
        val form = find(user, pw)!!
        assertEquals(listOf(user.index), form.usernames)
        assertEquals(listOf(pw.index), form.passwords)
        assertEquals("dashboard.render.com", form.webDomain)
        assertEquals(pw.index, form.focused)
    }

    // The Riot app's code step as Android reported it: one box per digit,
    // and only the focused box in the structure.
    private fun codeBox(focused: Boolean = false, domain: String? = null) = field(
        hints = listOf("new-password"),
        inputType = 0,
        hint = "",
        html = mapOf("maxlength" to "1", "name" to "", "label" to "", "id" to "", "type" to "text"),
        focused = focused,
        domain = domain,
    )

    @Test
    fun aFocusedOneCharacterBoxIsASplitCodeField() {
        val box = codeBox(focused = true, domain = "")
        val form = find(box)!!
        assertEquals(listOf(box.index), form.otps)
        assertTrue(form.otpOnly)
    }

    @Test
    fun aNativeAppsOneCharacterBoxIsASplitCodeFieldToo() {
        // A native EditText reports its length limit as maxTextLength, not HTML.
        val number = android.text.InputType.TYPE_CLASS_NUMBER
        val boxes = List(4) { field(inputType = number, maxTextLength = 1, focused = it == 0) }
        val form = find(*boxes.toTypedArray())!!
        assertEquals(listOf(boxes[0].index), form.otps)
    }


    @Test
    fun aRowOfOneCharacterBoxesIsFilledFromTheFirst() {
        val boxes = List(6) { codeBox(focused = it == 2, domain = "example.com") }
        val form = find(*boxes.toTypedArray())!!
        assertEquals(listOf(boxes[0].index), form.otps)
    }

    @Test
    fun aOneCharacterBoxIsNotACodeUnlessFocusedOnAScreenWithoutALogin() {
        assertNull(find(codeBox()))
        val user = field(inputType = email)
        val pw = field(inputType = password, focused = true)
        val form = find(user, codeBox(), pw)!!
        assertEquals(emptyList<Int>(), form.otps)
        assertEquals(listOf(pw.index), form.passwords)
    }

    @Test
    fun aLoneNewPasswordWithoutAUsernameIsNotFilled() {
        // A "choose a new password" step after a reset link.
        assertNull(find(field(html = mapOf("type" to "password", "autocomplete" to "new-password"))))
    }

    @Test
    fun aChangePasswordFormOffersOnlyTheCurrentPassword() {
        val current = field(html = mapOf("type" to "password", "autocomplete" to "current-password"))
        val next = field(html = mapOf("type" to "password", "autocomplete" to "new-password"))
        val form = find(current, next)!!
        assertEquals(listOf(current.index), form.passwords)
        // With a username too, the new password is still never the login's.
        val user = field(inputType = email)
        assertEquals(listOf(current.index), find(user, current, next)!!.passwords)
    }

    @Test
    fun nothingFillable() {
        assertNull(find())
        assertNull(find(field(id = "search", hint = "Search"), field(id = "comment")))
    }

    @Test
    fun anUnlabelledTextFieldBeforeAPasswordIsTheUsername() {
        val other = field()
        val user = field()
        val pw = field(inputType = password)
        val form = find(other, user, pw)!!
        assertEquals(listOf(user.index), form.usernames)
    }

    @Test
    fun theUnlabelledFallbackSkipsHiddenFieldsAndRefusesASearchBox() {
        val user = field()
        val hidden = field(visible = false)
        val pw = field(inputType = password)
        assertEquals(listOf(user.index), find(user, hidden, pw)!!.usernames)

        val search = field(id = "search")
        val pw2 = field(inputType = password)
        assertEquals(emptyList<Int>(), find(search, pw2)!!.usernames)
    }

    @Test
    fun theUsernameIsTheBestOneBeforeThePassword() {
        val weak = field(id = "phone", hint = "Phone")
        val strong = field(hints = listOf("username"))
        val pw = field(inputType = password)
        val after = field(inputType = email)
        assertEquals(FieldRole.USERNAME, FieldClassifier.classify(weak).role)
        assertEquals(listOf(strong.index), find(weak, strong, pw, after)!!.usernames)
    }

    @Test
    fun thousandsOfInputsStayFast() {
        val fields = List(5_000) { if (it % 2 == 0) field(id = "field$it") else field(inputType = password) }
        // The fastest of a few runs, so a cold JIT or a GC pause does not decide.
        val ms = List(5) { measureTimeMillis { LoginFormFinder.find(fields) } }.min()
        assertTrue("took $ms ms", ms < 50)
    }
}
