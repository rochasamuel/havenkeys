package net.havenkeys.android.autofill

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
}
