package net.havenkeys.android.autofill

import android.text.InputType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.IdentityRole

class IdentityFieldClassifierTest {
    private fun role(f: FieldFacts) = IdentityFieldClassifier.roleOf(f)?.role

    @Test
    fun androidHintsAndAutocomplete() {
        assertEquals(IdentityRole.FIRST_NAME, role(field(hints = listOf("personGivenName"))))
        assertEquals(IdentityRole.POSTAL_CODE, role(field(hints = listOf("postalCode"))))
        assertEquals(IdentityRole.CITY, role(field(html = mapOf("autocomplete" to "shipping address-level2"))))
        assertEquals(IdentityRole.BIRTH_DATE, role(date(hints = listOf("birthDateFull"))))
        assertTrue(IdentityFieldClassifier.roleOf(field(hints = listOf("postalCode")))!!.byHint)
    }

    @Test
    fun inputTypesAndWords() {
        assertEquals(IdentityRole.EMAIL, role(field(inputType = email)))
        assertEquals(IdentityRole.PHONE, role(field(inputType = InputType.TYPE_CLASS_PHONE)))
        assertEquals(IdentityRole.CPF, role(field(hint = "CPF")))
        assertEquals(IdentityRole.POSTAL_CODE, role(field(id = "cep")))
        assertEquals(IdentityRole.LAST_NAME, role(field(hint = "Sobrenome")))
        assertEquals(IdentityRole.STATE, role(list(listOf("SP", "RJ"), id = "estado")))
    }

    @Test
    fun compoundLabelsAreNotGuessed() {
        assertNull(role(field(hint = "Cidade de nascimento")))
        assertNull(role(field(hint = "Número do documento")))
        assertNull(role(field(hint = "Endereço da empresa")))
    }

    @Test
    fun cardsPasswordsAndOtherDataAreRefused() {
        assertNull(role(field(hint = "Nome impresso no cartão")))
        assertNull(role(field(hints = listOf("creditCardNumber"))))
        assertNull(role(field(hint = "Name", inputType = password)))
        assertNull(role(field(hint = "Nome da mãe")))
        assertNull(role(field(html = mapOf("autocomplete" to "one-time-code"), hint = "Phone")))
        assertNull(role(date(hints = listOf("postalCode"))))
    }

    @Test
    fun documentsAreTheFourDocumentRoles() {
        assertTrue(IdentityRole.CPF.isDocument)
        assertTrue(IdentityRole.DRIVERS_LICENSE.isDocument)
        assertFalse(IdentityRole.EMAIL.isDocument)
    }
}
