package net.havenkeys.android.autofill

import java.time.LocalDate
import uniffi.havenkeys_mobile.CardRole
import uniffi.havenkeys_mobile.IdentityRole

/** A value in the shape its field takes. A secret: never printed. */
sealed interface Shaped {
    data class Text(val text: String) : Shaped {
        override fun toString() = "Text(…)"
    }

    data class Pick(val index: Int) : Shaped

    data class Date(val epochMillis: Long) : Shaped
}

/**
 * Rust's whole values, shaped to one field (card-fill.ts, identity-fill.ts).
 * Null when the field cannot take the value: it is left empty, never cut.
 */
object ValueShaper {
    fun card(f: FieldFacts, field: CardField, byRole: Map<CardRole, String>): Shaped? {
        val month = byRole[CardRole.EXPIRY_MONTH]
        val year = byRole[CardRole.EXPIRY_YEAR]
        return when (field.kind) {
            CardKind.EXPIRY -> if (month != null && year != null) expiry(f, month, year) else null
            CardKind.EXPIRY_MONTH -> month?.let { monthIn(f, it) }
            CardKind.EXPIRY_YEAR -> year?.let { yearIn(f, it) }
            CardKind.NUMBER -> byRole[CardRole.NUMBER]?.let { number(f, field.slice, it) }
            CardKind.BRAND ->
                byRole[CardRole.BRAND]?.takeIf { f.isList }?.let { pick(OptionMatch.brand(f.options, it)) }
            else -> byRole[roleOf(field.kind)]?.let { fit(f, it) }
        }
    }

    fun identity(f: FieldFacts, role: IdentityRole, value: String): Shaped? = when {
        f.isList -> pick(OptionMatch.identity(f.options, role, value))
        f.isDate -> IdentityShapes.isoDate(value)
            ?.takeIf { role == IdentityRole.BIRTH_DATE }
            ?.let(IdentityShapes::dateOf)
        f.htmlAttributes["type"].equals("date", ignoreCase = true) ->
            IdentityShapes.isoDate(value)?.let { Shaped.Text(value) }
        role == IdentityRole.BIRTH_DATE ->
            IdentityShapes.birthDate(CardFormats.hintOf(f), value)?.let(Shaped::Text)
        role == IdentityRole.PHONE -> IdentityShapes.phone(f, value)?.let(Shaped::Text)
        else -> fit(f, value)
    }

    private fun monthIn(f: FieldFacts, month: String): Shaped? =
        if (f.isList) pick(OptionMatch.month(f.options, month)) else Shaped.Text(CardFormats.month(f, month))

    private fun yearIn(f: FieldFacts, year: String): Shaped? =
        if (f.isList) pick(OptionMatch.year(f.options, year)) else Shaped.Text(CardFormats.year(f, year))

    private fun expiry(f: FieldFacts, month: String, year: String): Shaped? = if (f.isDate) {
        val m = month.toIntOrNull()
        val y = year.toIntOrNull()
        if (m != null && y != null) {
            runCatching { LocalDate.of(y, m, 1) }.getOrNull()?.let(IdentityShapes::dateOf)
        } else {
            null
        }
    } else {
        Shaped.Text(CardFormats.expiry(f, month, year))
    }

    private fun number(f: FieldFacts, slice: IntRange?, digits: String): Shaped? = when {
        slice == null -> Shaped.Text(CardFormats.number(f, digits))
        slice.first >= digits.length -> null
        else -> Shaped.Text(digits.substring(slice.first, minOf(slice.last + 1, digits.length)))
    }

    private fun roleOf(kind: CardKind): CardRole? = when (kind) {
        CardKind.CARDHOLDER_NAME -> CardRole.CARDHOLDER_NAME
        CardKind.CARDHOLDER_GIVEN_NAME -> CardRole.CARDHOLDER_GIVEN_NAME
        CardKind.CARDHOLDER_FAMILY_NAME -> CardRole.CARDHOLDER_FAMILY_NAME
        CardKind.VERIFICATION_NUMBER -> CardRole.VERIFICATION_NUMBER
        else -> null
    }

    private fun fit(f: FieldFacts, value: String): Shaped? {
        val max = maxLengthOf(f)
        return Shaped.Text(value).takeIf { max < 0 || value.length <= max }
    }

    private fun pick(index: Int): Shaped? = if (index >= 0) Shaped.Pick(index) else null
}
