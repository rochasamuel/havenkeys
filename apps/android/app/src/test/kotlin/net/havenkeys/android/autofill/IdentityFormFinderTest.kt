package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.IdentityRole

class IdentityFormFinderTest {
    @Test
    fun anAddressFormInTheFocusedFrame() {
        val fields = listOf(
            field(hint = "Nome completo", focused = true, domain = "shop.example.com", scheme = "https"),
            field(id = "cep", domain = "shop.example.com", scheme = "https"),
            field(hint = "Cidade", domain = "shop.example.com", scheme = "https"),
            field(hint = "Cidade", domain = "ads.example.net", scheme = "https"),
        )
        val form = IdentityFormFinder.find(fields)!!
        assertEquals(listOf(IdentityRole.FULL_NAME, IdentityRole.POSTAL_CODE, IdentityRole.CITY), form.roles)
        assertEquals("shop.example.com", form.webDomain)
    }

    @Test
    fun aLoneEmailOrUsernameIsALoginStepNotAnIdentity() {
        assertNull(IdentityFormFinder.find(listOf(field(inputType = email, focused = true))))
        assertNull(IdentityFormFinder.find(listOf(field(hints = listOf("username"), focused = true))))
    }

    @Test
    fun aLoneFieldNamedByAHintQualifies() {
        val form = IdentityFormFinder.find(listOf(field(hints = listOf("postalCode"), focused = true)))!!
        assertEquals(listOf(IdentityRole.POSTAL_CODE), form.roles)
    }

    @Test
    fun theFocusedFieldMustBeAnIdentityField() {
        val fields = listOf(field(hint = "Cidade"), field(id = "cep"), field(hint = "Coupon", focused = true))
        assertNull(IdentityFormFinder.find(fields))
    }
}
