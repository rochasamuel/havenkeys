package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class FormRouterTest {
    @Test
    fun aFocusedCardFieldGoesToCards() {
        val routed = FormRouter.route(listOf(field(hint = "Card number", focused = true), field(id = "cvv")))
        assertTrue(routed is Routed.Card)
    }

    @Test
    fun aSignInGoesToLogins() {
        val routed = FormRouter.route(listOf(field(inputType = email, focused = true), field(inputType = password)))
        assertTrue(routed is Routed.Login)
    }

    @Test
    fun anEmailFirstStepGoesToLogins() {
        assertTrue(FormRouter.route(listOf(field(inputType = email, focused = true))) is Routed.Login)
    }

    @Test
    fun aLoneCpfLoginStepStaysALogin() {
        // gov.br's first step: one CPF box, the password on the next screen.
        val routed = FormRouter.route(listOf(field(id = "cpf", hint = "CPF", focused = true)))
        assertTrue(routed is Routed.Login)
    }

    @Test
    fun anAddressFormGoesToTheIdentity() {
        val routed = FormRouter.route(
            listOf(field(hint = "Nome completo", focused = true), field(id = "cep"), field(hint = "Cidade")),
        )
        assertTrue(routed is Routed.Identity)
    }

    @Test
    fun aSignUpGoesToTheIdentityAndKeepsItsSave() {
        val routed = FormRouter.route(
            listOf(
                field(hint = "Full name", focused = true),
                field(inputType = email),
                field(inputType = password, hint = "Create password"),
                field(inputType = password, hint = "Confirm password"),
            ),
        )
        assertTrue(routed is Routed.Identity)
        assertNotNull((routed as Routed.Identity).save)
    }

    @Test
    fun nothingFillableRoutesNowhere() {
        assertNull(FormRouter.route(listOf(field(hint = "Search", html = mapOf("type" to "search"), focused = true))))
    }

    private val webEmail = mapOf("type" to "email", "autocomplete" to "email")
    private val webName = mapOf("type" to "text", "autocomplete" to "name")
    private val webPw = mapOf("type" to "password")

    @Test
    fun anEmailFirstStepBesideANewsletterFormIsALogin() {
        val user = field(html = webEmail, group = htmlForm(1), focused = true)
        val name = field(html = webName, group = htmlForm(2))
        val news = field(html = webEmail, group = htmlForm(2))
        val routed = FormRouter.route(listOf(user, name, news))
        assertTrue(routed is Routed.Login)
        assertEquals(listOf(user.index), (routed as Routed.Login).form?.usernames)
        assertEquals(user.index, routed.save?.username)
    }

    @Test
    fun withoutGroupsAnEmailBesideOtherIdentityFieldsStaysTheIdentity() {
        // No form in the structure, no password: no login context, so the identity as before.
        val routed = FormRouter.route(
            listOf(field(html = webEmail, focused = true), field(html = webName), field(html = webEmail)),
        )
        assertTrue(routed is Routed.Identity)
    }

    @Test
    fun anIdentityFormWithoutAPasswordStaysTheIdentity() {
        val name = field(html = webName, group = htmlForm(1))
        val mail = field(html = webEmail, group = htmlForm(1), focused = true)
        val phone = field(html = mapOf("type" to "tel", "autocomplete" to "tel"), group = htmlForm(1))
        val routed = FormRouter.route(listOf(name, mail, phone))
        assertTrue(routed is Routed.Identity)
        val indices = (routed as Routed.Identity).form.fields.map { it.index }
        assertEquals(listOf(name.index, mail.index, phone.index), indices)
    }

    @Test
    fun aNewsletterBesideASignInGetsTheIdentityNotTheLogin() {
        val user = field(html = webEmail, group = htmlForm(1))
        val pw = field(html = webPw, group = htmlForm(1))
        val name = field(html = webName, group = htmlForm(2))
        val news = field(html = webEmail, group = htmlForm(2), focused = true)
        val routed = FormRouter.route(listOf(user, pw, name, news))
        assertTrue(routed is Routed.Identity)
        assertEquals(listOf(name.index, news.index), (routed as Routed.Identity).form.fields.map { it.index })
        assertNull(routed.save)
    }

    @Test
    fun aLoneNewsletterEmailBesideASignInIsTheIdentitysEmail() {
        val user = field(html = webEmail, group = htmlForm(1))
        val pw = field(html = webPw, group = htmlForm(1))
        val news = field(html = webEmail, group = htmlForm(2), focused = true)
        val routed = FormRouter.route(listOf(user, pw, news))
        assertTrue(routed is Routed.Identity)
        assertEquals(listOf(news.index), (routed as Routed.Identity).form.fields.map { it.index })
    }

    @Test
    fun aNewsletterEmailAloneOnAPageIsTheIdentitysEmail() {
        val news = field(html = webEmail + ("name" to "newsletter"), focused = true)
        assertTrue(FormRouter.route(listOf(news)) is Routed.Identity)
    }

    @Test
    fun aSignUpBesideASignInIsNoLoginFill() {
        val user = field(html = webEmail, group = htmlForm(1))
        val pw = field(html = webPw, group = htmlForm(1))
        val name = field(html = webName, group = htmlForm(2))
        val mail = field(html = webEmail, group = htmlForm(2), focused = true)
        val create = field(html = mapOf("type" to "password", "autocomplete" to "new-password"), group = htmlForm(2))
        val confirm = field(html = webPw, hint = "Confirm password", group = htmlForm(2))
        val routed = FormRouter.route(listOf(user, pw, name, mail, create, confirm))
        assertTrue(routed is Routed.Identity)
        routed as Routed.Identity
        assertEquals(listOf(name.index, mail.index), routed.form.fields.map { it.index })
        assertEquals(create.index, routed.save?.password)
    }

    @Test
    fun aSearchBoxBesideASignInRoutesNowhere() {
        val user = field(html = webEmail, group = htmlForm(1))
        val pw = field(html = webPw, group = htmlForm(1))
        val search = field(html = mapOf("type" to "search", "name" to "q"), group = htmlForm(2), focused = true)
        assertNull(FormRouter.route(listOf(user, pw, search)))
    }

    @Test
    fun theFocusedSignInOfTwoIsRouted() {
        val news = field(html = webEmail, group = htmlForm(2))
        val user = field(html = webEmail, group = htmlForm(1), focused = true)
        val pw = field(html = webPw, group = htmlForm(1))
        val routed = FormRouter.route(listOf(news, user, pw)) as Routed.Login
        assertEquals(listOf(user.index), routed.form?.usernames)
        assertEquals(listOf(pw.index), routed.form?.passwords)
    }

    @Test
    fun thousandsOfMixedInputsRouteFast() {
        // CLAUDE.md attack 7 for what a request runs: cards, logins, saves and identity.
        assertScalesLinearly(
            page = { n ->
                List(n) {
                    when (it % 6) {
                        0 -> field(id = "field$it")
                        1 -> field(inputType = password)
                        2 -> field(hint = "Card number")
                        3 -> field(id = "cep", hint = "Cidade")
                        4 -> list(listOf("01", "02", "03"), id = "exp-month")
                        else -> field(inputType = email, focused = it == 5)
                    }
                }
            },
            work = { FormRouter.route(it) },
        )
    }
}
