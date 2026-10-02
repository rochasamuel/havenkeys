package net.havenkeys.android.autofill

import android.text.InputType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class TextMatchTest {
    @Test
    fun normalizeSplitsCamelCaseAndDropsAccents() {
        assertEquals("login email endereco", normalize("loginEmail_Endereço"))
        assertEquals("", normalize(null))
        assertEquals("numero do cartao", normalizeAll("Número", null, "", "do cartão"))
    }

    @Test
    fun phrasesMatchWholeWordsOnly() {
        assertTrue(hasAny("your card number", listOf("card number")))
        assertFalse(hasAny("cardnumber", listOf("card number")))
        assertFalse(hasAny("", listOf("card")))
    }

    @Test
    fun maxLengthPrefersTheHtmlAttribute() {
        assertEquals(4, maxLengthOf(field(html = mapOf("maxlength" to "4"), maxTextLength = 9)))
        assertEquals(9, maxLengthOf(field(maxTextLength = 9)))
        assertEquals(-1, maxLengthOf(field()))
        assertEquals(-1, maxLengthOf(field(html = mapOf("maxlength" to "x"))))
    }

    @Test
    fun passwordInputTypesAreRecognised() {
        assertTrue(isPasswordInputType(password))
        assertTrue(isPasswordInputType(InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_VARIATION_PASSWORD))
        assertFalse(isPasswordInputType(InputType.TYPE_CLASS_TEXT))
    }
}
