package net.havenkeys.android.autofill

/** card-fill.ts's text shapes for expiry, month, year and number. */
internal object CardFormats {
    /** Month and year in one field: the placeholder's format, else the pattern's, else by length, else MM/YY. */
    fun expiry(f: FieldFacts, month: String, year: String): String {
        val mm = month.padStart(2, '0')
        val yy = year.takeLast(2)
        val max = maxLengthOf(f)
        fun fits(s: String) = max < 0 || s.length <= max
        val fromHint = EXPIRY_HINT.find(hintOf(f))?.let { m ->
            "$mm${m.groupValues[1]}${if (m.groupValues[2].length == LONG_YEAR) year else yy}"
        }
        val fromPattern = if (fromHint == null) {
            patternShape(f)?.let { (sep, long) -> "$mm$sep${if (long) year else yy}" }
        } else {
            null
        }
        return listOfNotNull(fromHint, fromPattern).firstOrNull(::fits) ?: when (max) {
            SHORT_EXPIRY -> "$mm$yy"
            LONG_EXPIRY -> "$mm/$year"
            else -> "$mm/$yy"
        }
    }

    fun month(f: FieldFacts, month: String): String =
        if (maxLengthOf(f) == PAIR || hasAny(normalize(hintOf(f)), listOf("mm"))) month.padStart(2, '0') else month

    fun year(f: FieldFacts, year: String): String =
        if (maxLengthOf(f) == PAIR || hintOf(f).trim() in SHORT_YEAR_HINTS) year.takeLast(2) else year

    /** Digits, grouped with spaces when the placeholder shows groups (Amex 4-6-5). */
    fun number(f: FieldFacts, digits: String): String {
        val max = maxLengthOf(f)
        val amex = digits.length == AMEX_LENGTH && (digits.startsWith("34") || digits.startsWith("37"))
        val spaced = if (amex) {
            val groups = listOf(
                digits.take(GROUP),
                digits.substring(GROUP, AMEX_SECOND_END),
                digits.substring(AMEX_SECOND_END),
            )
            groups.joinToString(" ")
        } else {
            digits.chunked(GROUP).joinToString(" ")
        }
        val grouped = GROUPED_HINT.containsMatchIn(hintOf(f)) && (max < 0 || spaced.length <= max)
        return if (grouped) spaced else digits
    }

    /** What the user reads as the format: the placeholder, else the native hint. */
    fun hintOf(f: FieldFacts): String =
        (f.htmlAttributes["placeholder"] ?: f.hint).orEmpty().take(MAX_TEXT_CHARS).lowercase()

    /** A simple digit `pattern` such as `\d{2}\s?/\s?\d{2}`: its separator and whether the year has 4 digits. */
    private fun patternShape(f: FieldFacts): Pair<String, Boolean>? {
        val flat = f.htmlAttributes["pattern"].orEmpty().take(MAX_PATTERN)
            .replace(ANCHORS, "")
            .replace(OPTIONAL_SPACE, "")
            .replace(ESCAPED_SEPARATOR, "$1")
            .replace(COUNTED_DIGIT) { "d".repeat(it.groupValues[1].toInt()) }
            .replace(ONE_DIGIT, "d")
        return PATTERN_SHAPE.matchEntire(flat)?.destructured
            ?.let { (_, sep, year) -> sep to (year.length == LONG_YEAR) }
    }

    private const val PAIR = 2
    private const val GROUP = 4
    private const val LONG_YEAR = 4
    private const val SHORT_EXPIRY = 4
    private const val LONG_EXPIRY = 7
    private const val AMEX_LENGTH = 15
    private const val AMEX_SECOND_END = 10
    private const val MAX_PATTERN = 100
    private val SHORT_YEAR_HINTS = setOf("yy", "aa")
    private val EXPIRY_HINT = Regex("mm(\\s*[/.-]\\s*|)(yyyy|aaaa|yy|aa)(?![a-z])")
    private val GROUPED_HINT = Regex("\\d{4}\\s\\d|[x•*]{4}\\s[x•*]")
    private val ANCHORS = Regex("^\\^|\\$$")
    private val OPTIONAL_SPACE = Regex("\\\\s\\??")
    private val ESCAPED_SEPARATOR = Regex("\\\\([/.-])")
    private val COUNTED_DIGIT = Regex("(?:\\\\d|\\[0-9])\\{(\\d)}")
    private val ONE_DIGIT = Regex("\\\\d|\\[0-9]")
    private val PATTERN_SHAPE = Regex("(dd)([/.-]?)(dd|dddd)")
}
