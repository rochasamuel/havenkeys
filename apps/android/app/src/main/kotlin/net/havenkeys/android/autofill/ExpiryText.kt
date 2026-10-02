package net.havenkeys.android.autofill

import java.util.Locale

/** A typed or chosen expiry as Rust takes it, `"YYYY-MM"`. Rust validates it again. */
object ExpiryText {
    fun of(combined: String?, month: String?, year: String?): String? =
        combined?.trim()?.let(::fromCombined) ?: fromParts(month, year)

    /** "04/33", "0433", "04 / 2033", or a date field's "2033-04-01". */
    private fun fromCombined(text: String): String? {
        val iso = ISO_DATE.matchEntire(text)
        val typed = COMBINED.matchEntire(text)
        return when {
            iso != null -> format(iso.groupValues[2].toIntOrNull(), iso.groupValues[1])
            typed != null -> format(typed.groupValues[1].toIntOrNull(), typed.groupValues[2])
            else -> null
        }
    }

    private fun fromParts(month: String?, year: String?): String? =
        if (month == null || year == null) null else format(OptionMatch.monthNumber(month), year.trim())

    private fun format(month: Int?, year: String): String? {
        val full = when (year.length) {
            2 -> year.toIntOrNull()?.plus(CENTURY)
            LONG_YEAR -> year.toIntOrNull()
            else -> null
        }
        return if (month != null && month in 1..MONTHS && full != null) {
            String.format(Locale.ROOT, "%04d-%02d", full, month)
        } else {
            null
        }
    }

    private const val CENTURY = 2000
    private const val LONG_YEAR = 4
    private const val MONTHS = 12
    private val ISO_DATE = Regex("(\\d{4})-(\\d{2})-\\d{2}")
    private val COMBINED = Regex("(\\d{1,2})\\s*[/.\\-]?\\s*(\\d{4}|\\d{2})")
}
