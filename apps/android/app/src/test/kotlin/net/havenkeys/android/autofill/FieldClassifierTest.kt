package net.havenkeys.android.autofill

import android.text.InputType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class FieldClassifierTest {
    private fun role(f: FieldFacts) = FieldClassifier.classify(f).role

    @Test
    fun autofillHintsWin() {
        assertEquals(FieldRole.USERNAME, role(field(hints = listOf("username"))))
        assertEquals(FieldRole.USERNAME, role(field(hints = listOf("emailAddress"))))
        assertEquals(FieldRole.PASSWORD, role(field(hints = listOf("password"))))
        assertEquals(FieldRole.NEW_PASSWORD, role(field(hints = listOf("newPassword"), inputType = password)))
        assertEquals(FieldRole.OTP, role(field(hints = listOf("smsOTPCode"))))
    }

    @Test
    fun htmlAutocompleteIsRead() {
        val current = field(html = mapOf("type" to "password", "autocomplete" to "current-password"))
        val new = field(html = mapOf("type" to "password", "autocomplete" to "new-password"))
        assertEquals(FieldRole.PASSWORD, role(current))
        assertEquals(FieldRole.NEW_PASSWORD, role(new))
        assertEquals(FieldRole.OTP, role(field(html = mapOf("autocomplete" to "one-time-code"))))
        assertEquals(FieldRole.USERNAME, role(field(html = mapOf("type" to "email"))))
    }

    @Test
    fun inputTypeAndWordsAreSignalsTooInPortuguese() {
        assertEquals(FieldRole.PASSWORD, role(field(inputType = webPassword)))
        assertEquals(FieldRole.USERNAME, role(field(inputType = email)))
        assertEquals(FieldRole.USERNAME, role(field(id = "login_usuario")))
        assertEquals(FieldRole.OTP, role(field(hint = "Código de verificação")))
    }

    @Test
    fun hiddenOrDisabledFieldsAreUnknown() {
        assertEquals(FieldRole.UNKNOWN, role(field(hints = listOf("password"), visible = false)))
        assertEquals(FieldRole.UNKNOWN, role(field(hints = listOf("password"), enabled = false)))
    }

    @Test
    fun aSearchBoxIsNotAUsername() {
        assertEquals(FieldRole.UNKNOWN, role(field(id = "search", hint = "Search")))
        assertEquals(FieldRole.UNKNOWN, role(field(html = mapOf("type" to "search", "name" to "user"))))
    }

    @Test
    fun wordsMatchWholeWordsNotSubstrings() {
        // "passenger" is not "pass", "username" inside camelCase still is.
        assertEquals(FieldRole.UNKNOWN, role(field(id = "passenger_name")))
        assertEquals(FieldRole.USERNAME, role(field(id = "editUsername")))
    }

    @Test
    fun aNewPasswordAutocompleteOnATextFieldIsNotAPassword() {
        // Sites put autocomplete=new-password on any field to keep browsers away.
        val cpf = field(html = mapOf("type" to "text", "name" to "cpf", "autocomplete" to "new-password"))
        assertEquals(FieldRole.USERNAME, role(cpf))
    }

    @Test
    fun aNewPasswordAutocompleteThatSaysCurrentIsTheCurrentPassword() {
        val f = field(
            html = mapOf("type" to "password", "autocomplete" to "new-password", "placeholder" to "Senha atual"),
        )
        assertEquals(FieldRole.PASSWORD, role(f))
    }

    @Test
    fun aConfirmationFieldIsRecognised() {
        assertEquals(FieldRole.CONFIRM_PASSWORD, role(field(inputType = password, hint = "Confirme a senha")))
        assertEquals(FieldRole.NEW_PASSWORD, role(field(inputType = password, hint = "Nova senha")))
    }

    @Test
    fun cardAndAddressFieldsAreNotLogins() {
        assertEquals(FieldRole.UNKNOWN, role(field(html = mapOf("autocomplete" to "cc-number", "name" to "card_code"))))
        val zip = field(html = mapOf("autocomplete" to "postal-code", "name" to "login_zip"))
        assertEquals(FieldRole.UNKNOWN, role(zip))
        assertEquals(FieldRole.UNKNOWN, role(field(id = "postal_code", hint = "Postal code")))
    }

    @Test
    fun nonInputHtmlIsUnknown() {
        val f = field(html = mapOf("type" to "hidden", "name" to "username"))
        assertEquals(FieldRole.UNKNOWN, role(f))
        val button = field(html = mapOf("type" to "submit", "name" to "login"))
        assertEquals(FieldRole.UNKNOWN, role(button))
    }

    @Test
    fun aNumericCodeFieldIsAnOtp() {
        val f = field(
            inputType = InputType.TYPE_CLASS_NUMBER,
            hint = "Enter the 6-digit code",
            html = mapOf("maxlength" to "6"),
        )
        assertEquals(FieldRole.OTP, role(f))
    }

    @Test
    fun confidenceIsBounded() {
        val c = FieldClassifier.classify(field(hints = listOf("username"), inputType = email, id = "username"))
        assertTrue(c.confidence in 1..100)
        assertEquals(0, FieldClassifier.classify(field()).confidence)
    }

    @Test
    fun aListOrADateIsNeverALoginField() {
        assertEquals(FieldRole.UNKNOWN, role(list(listOf("a", "b"), id = "username")))
        assertEquals(FieldRole.UNKNOWN, role(date(hints = listOf("username"))))
    }
}
