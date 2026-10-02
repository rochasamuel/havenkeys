package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class CardFieldClassifierTest {
    private fun kind(f: FieldFacts, strong: Boolean = false) = CardFieldClassifier.kindOf(f, strong)?.kind

    @Test
    fun androidHintsAndAutocompleteName() {
        assertEquals(CardKind.NUMBER, kind(field(hints = listOf("creditCardNumber"))))
        assertEquals(CardKind.VERIFICATION_NUMBER, kind(field(hints = listOf("creditCardSecurityCode"))))
        assertEquals(CardKind.EXPIRY, kind(field(hints = listOf("creditCardExpirationDate"))))
        assertEquals(CardKind.EXPIRY_MONTH, kind(field(html = mapOf("autocomplete" to "billing cc-exp-month"))))
        assertEquals(CardKind.CARDHOLDER_NAME, kind(field(html = mapOf("autocomplete" to "cc-name"))))
        assertTrue(CardFieldClassifier.kindOf(field(hints = listOf("creditCardNumber")), false)!!.byHint)
    }

    @Test
    fun wordsInEnglishAndPortuguese() {
        assertEquals(CardKind.NUMBER, kind(field(hint = "Número do cartão")))
        assertEquals(CardKind.VERIFICATION_NUMBER, kind(field(id = "cvv")))
        assertEquals(CardKind.EXPIRY, kind(field(hint = "Validade (MM/AA)")))
        assertEquals(CardKind.CARDHOLDER_NAME, kind(field(hint = "Nome impresso no cartão")))
    }

    @Test
    fun ambiguousWordsNeedAStrongFrame() {
        val code = field(hint = "Security code", maxTextLength = 4)
        assertNull(kind(code))
        assertEquals(CardKind.VERIFICATION_NUMBER, kind(code, strong = true))
        assertNull(kind(field(hint = "Titular")))
    }

    @Test
    fun aPasswordBoxIsOnlyEverTheCode() {
        assertEquals(CardKind.VERIFICATION_NUMBER, kind(field(id = "cvc", inputType = password)))
        assertNull(kind(field(hint = "Card number", inputType = password)))
    }

    @Test
    fun listsAreMonthYearOrBrandOnly() {
        val months = listOf("Mês") + (1..12).map { "%02d".format(it) }
        val years = listOf("Ano") + (2026..2040).map { it.toString() }
        assertEquals(CardKind.EXPIRY_MONTH, kind(list(months, id = "month"), strong = true))
        assertEquals(CardKind.EXPIRY_MONTH, kind(list(months), strong = true))
        assertEquals(CardKind.EXPIRY_YEAR, kind(list(years), strong = true))
        assertNull(kind(list(months)))
        assertNull(kind(list(listOf("a", "b"), id = "card number")))
        assertEquals(CardKind.BRAND, kind(list(listOf("Visa", "Mastercard"), id = "bandeira")))
    }

    @Test
    fun aDateFieldIsOnlyTheExpiry() {
        assertEquals(CardKind.EXPIRY, kind(date(hints = listOf("creditCardExpirationDate"))))
        assertNull(kind(date(hints = listOf("creditCardNumber"))))
    }

    @Test
    fun otherCardsAndCodesAreRefused() {
        assertNull(kind(field(hint = "Gift card number")))
        assertNull(kind(field(hint = "Número de parcelas")))
        assertNull(kind(field(html = mapOf("autocomplete" to "one-time-code"), id = "cvv")))
        assertNull(kind(field(hint = "Validade", id = "data_nascimento")))
        assertNull(kind(field(hint = "Titular", id = "cpf"), strong = true))
    }
}
