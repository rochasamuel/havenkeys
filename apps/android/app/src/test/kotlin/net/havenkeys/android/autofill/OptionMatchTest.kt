package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.IdentityRole

class OptionMatchTest {
    @Test
    fun aMonthListMatchesByNumberOrName() {
        val numbered = listOf("Mês") + (1..12).map { "%02d - %s".format(it, listOf("Janeiro", "Fevereiro", "Março",
            "Abril", "Maio", "Junho", "Julho", "Agosto", "Setembro", "Outubro", "Novembro", "Dezembro")[it - 1]) }
        assertEquals(3, OptionMatch.month(numbered, "3"))
        assertEquals(12, OptionMatch.month(numbered, "12"))
        assertEquals(2, OptionMatch.month(listOf("Jan", "Feb", "Mar"), "3"))
        assertEquals(-1, OptionMatch.month(listOf("Jan", "Feb", "Mar"), "5"))
        assertEquals(-1, OptionMatch.month(listOf("Month"), "3"))
    }

    @Test
    fun aYearListMatchesTwoOrFourDigits() {
        assertEquals(8, OptionMatch.year(listOf("Ano") + (2026..2040).map { "$it" }, "2033"))
        assertEquals(8, OptionMatch.year(listOf("AA") + (26..40).map { "$it" }, "2033"))
        assertEquals(-1, OptionMatch.year(listOf("2026"), "2033"))
    }

    @Test
    fun aBrandListMatchesCommonNames() {
        assertEquals(1, OptionMatch.brand(listOf("Selecione", "VISA", "Master"), "visa"))
        assertEquals(2, OptionMatch.brand(listOf("Selecione", "VISA", "Master"), "mastercard"))
        assertEquals(1, OptionMatch.brand(listOf("-", "American Express"), "amex"))
        assertEquals(-1, OptionMatch.brand(listOf("-", "Elo"), "other"))
    }

    @Test
    fun statesAndCountriesMatchTheirOtherNames() {
        assertEquals(2, OptionMatch.identity(listOf("UF", "RJ", "SP"), IdentityRole.STATE, "São Paulo"))
        assertEquals(1, OptionMatch.identity(listOf("Estado", "Distrito Federal"), IdentityRole.STATE, "DF"))
        assertEquals(1, OptionMatch.identity(listOf("País", "Brasil"), IdentityRole.COUNTRY, "Brazil"))
        assertEquals(-1, OptionMatch.identity(listOf("Cidade"), IdentityRole.CITY, "Brasília"))
    }

    @Test
    fun monthNumbersFromTextOrNames() {
        assertEquals(3, OptionMatch.monthNumber("03"))
        assertEquals(3, OptionMatch.monthNumber("Março"))
        assertEquals(12, OptionMatch.monthNumber("dez"))
        assertNull(OptionMatch.monthNumber("13"))
        assertNull(OptionMatch.monthNumber("Mês"))
    }
}
