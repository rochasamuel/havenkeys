package net.havenkeys.android.autofill

import android.text.InputType
import java.time.LocalDate
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.CardRole
import uniffi.havenkeys_mobile.IdentityRole

class ValueShaperTest {
    private val visa = mapOf(
        CardRole.NUMBER to "4111111111111111",
        CardRole.VERIFICATION_NUMBER to "123",
        CardRole.EXPIRY_MONTH to "4",
        CardRole.EXPIRY_YEAR to "2033",
        CardRole.CARDHOLDER_NAME to "Samuel Rocha",
        CardRole.BRAND to "visa",
    )

    private fun card(f: FieldFacts, kind: CardKind, slice: IntRange? = null) =
        ValueShaper.card(f, CardField(f.index, kind, false, slice), visa)

    private fun text(s: String) = Shaped.Text(s)

    @Test
    fun expiryFollowsThePlaceholderThePatternOrTheLength() {
        assertEquals(text("04/33"), card(field(html = mapOf("placeholder" to "MM/AA")), CardKind.EXPIRY))
        // The placeholder's spacing is kept, as card-fill.ts does.
        assertEquals(text("04 / 33"), card(field(html = mapOf("placeholder" to "MM / AA")), CardKind.EXPIRY))
        assertEquals(text("04/2033"), card(field(html = mapOf("placeholder" to "MM/YYYY")), CardKind.EXPIRY))
        assertEquals(text("0433"), card(field(html = mapOf("pattern" to "[0-9]{2}[0-9]{2}")), CardKind.EXPIRY))
        assertEquals(text("0433"), card(field(maxTextLength = 4), CardKind.EXPIRY))
        assertEquals(text("04/2033"), card(field(maxTextLength = 7), CardKind.EXPIRY))
        assertEquals(text("04/33"), card(field(), CardKind.EXPIRY))
    }

    @Test
    fun monthAndYearInTheirOwnFields() {
        assertEquals(text("04"), card(field(maxTextLength = 2), CardKind.EXPIRY_MONTH))
        assertEquals(text("4"), card(field(), CardKind.EXPIRY_MONTH))
        assertEquals(text("33"), card(field(maxTextLength = 2), CardKind.EXPIRY_YEAR))
        assertEquals(text("2033"), card(field(), CardKind.EXPIRY_YEAR))
        val months = (1..12).map { "%02d".format(it) }
        assertEquals(Shaped.Pick(3), card(list(months), CardKind.EXPIRY_MONTH))
    }

    @Test
    fun theNumberIsGroupedSlicedOrPlain() {
        assertEquals(text("4111111111111111"), card(field(), CardKind.NUMBER))
        assertEquals(
            text("4111 1111 1111 1111"),
            card(field(html = mapOf("placeholder" to "0000 0000 0000 0000")), CardKind.NUMBER),
        )
        assertEquals(text("1111"), card(field(maxTextLength = 4), CardKind.NUMBER, slice = 4 until 8))
    }

    @Test
    fun aCodeThatDoesNotFitIsLeftOut() {
        assertEquals(text("123"), card(field(maxTextLength = 3), CardKind.VERIFICATION_NUMBER))
        assertNull(card(field(maxTextLength = 2), CardKind.VERIFICATION_NUMBER))
    }

    @Test
    fun nothingIsEverCut() {
        assertNull(card(field(maxTextLength = 12), CardKind.NUMBER))
        assertNull(card(field(maxTextLength = 3), CardKind.EXPIRY))
        assertNull(card(field(maxTextLength = 3), CardKind.NUMBER, slice = 4 until 8))
        assertNull(card(field(maxTextLength = 1), CardKind.EXPIRY_YEAR))
        assertNull(card(field(), CardKind.NUMBER, slice = IntRange(5, 4)))
        assertNull(ValueShaper.identity(
            field(html = mapOf("placeholder" to "dd/mm/aaaa"), maxTextLength = 8),
            IdentityRole.BIRTH_DATE, "1990-03-12",
        ))
        assertNull(ValueShaper.identity(
            field(html = mapOf("type" to "date"), maxTextLength = 8), IdentityRole.BIRTH_DATE, "1990-03-12",
        ))
    }

    @Test
    fun aDatePickerGetsTheFirstDayOfTheMonth() {
        // Android's DatePicker reads the millis in the device's time zone: local midnight.
        val local = LocalDate.of(2033, 4, 1).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli()
        assertEquals(Shaped.Date(local), card(date(), CardKind.EXPIRY))
    }

    @Test
    fun theBrandGoesOnlyIntoAList() {
        assertEquals(Shaped.Pick(1), card(list(listOf("-", "Visa")), CardKind.BRAND))
        assertNull(card(field(), CardKind.BRAND))
    }

    @Test
    fun identityValuesInTheirShape() {
        assertEquals(text("12/03/1990"), ValueShaper.identity(
            field(html = mapOf("placeholder" to "dd/mm/aaaa")), IdentityRole.BIRTH_DATE, "1990-03-12",
        ))
        assertNull(ValueShaper.identity(field(), IdentityRole.BIRTH_DATE, "1990-03-12"))
        assertEquals(text("1990-03-12"), ValueShaper.identity(
            field(html = mapOf("type" to "date")), IdentityRole.BIRTH_DATE, "1990-03-12",
        ))
        assertEquals(text("61 99999-0000"), ValueShaper.identity(
            field(inputType = InputType.TYPE_CLASS_PHONE, maxTextLength = 13), IdentityRole.PHONE, "+55 61 99999-0000",
        ))
        assertNull(ValueShaper.identity(field(maxTextLength = 3), IdentityRole.CITY, "Brasília"))
        assertEquals(Shaped.Pick(1), ValueShaper.identity(list(listOf("UF", "DF")), IdentityRole.STATE, "DF"))
    }

    @Test
    fun aTextValueNeverPrintsItself() {
        assertEquals("Text(…)", text("4111111111111111").toString())
    }
}
