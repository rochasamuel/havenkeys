package net.havenkeys.android.autofill

/**
 * Reading and matching list options (card-kind.ts and card-fill.ts). The
 * labels come from the page or app: they are only compared against fixed
 * lists and numbers.
 */
internal object OptionMatch {
    /** Month names and abbreviations, English and Portuguese, normalized; index 0 = January. */
    val MONTHS: List<List<String>> = listOf(
        listOf("january", "jan", "janeiro"),
        listOf("february", "feb", "fevereiro", "fev"),
        listOf("march", "mar", "marco"),
        listOf("april", "apr", "abril", "abr"),
        listOf("may", "maio", "mai"),
        listOf("june", "jun", "junho"),
        listOf("july", "jul", "julho"),
        listOf("august", "aug", "agosto", "ago"),
        listOf("september", "sep", "sept", "setembro", "set"),
        listOf("october", "oct", "outubro", "out"),
        listOf("november", "nov", "novembro"),
        listOf("december", "dec", "dezembro", "dez"),
    )

    fun words(option: String): List<String> = normalize(option).split(' ').filter { it.isNotEmpty() }

    /** Exactly 12 month options plus at most one placeholder. */
    fun isMonthList(options: List<String>): Boolean {
        val scanned = options.take(MONTHS_IN_YEAR + EXTRA_SCAN)
        val months = scanned.count { option ->
            val w = words(option)
            w.isNotEmpty() && w.size <= MAX_MONTH_WORDS && w.none { NUMBER_WITH_LETTERS.matches(it) } &&
                w.any { MONTH_NUMBER.matches(it) || isMonthName(it) }
        }
        return months == MONTHS_IN_YEAR && scanned.size <= MONTHS_IN_YEAR + 1
    }

    /** At least 10 consecutive years (2 or 4 digits) plus at most one placeholder. */
    fun isYearList(options: List<String>): Boolean {
        val years = sortedSetOf<Int>()
        var other = 0
        for (option in options.take(YEAR_SCAN)) {
            val year = yearOf(option)
            if (year != null) years += year else other++
        }
        return other <= 1 && longestRun(years) >= MIN_YEAR_RUN
    }

    /** "2033" or "33" → 2033; 2-digit years start at 20, so "01".."12" is not a year. */
    fun yearOf(option: String): Int? {
        val w = words(option).singleOrNull()
        val yy = w?.let { YEAR.matchEntire(it) }?.groupValues?.get(1)?.toInt()
        return yy?.takeIf { it >= MIN_TWO_DIGIT_YEAR }?.plus(CENTURY)
    }

    private fun isMonthName(word: String) =
        word.length >= MIN_NAME_PREFIX &&
            MONTHS.any { names -> names.any { word.startsWith(it.take(MIN_NAME_PREFIX)) } }

    private fun longestRun(years: Set<Int>): Int {
        var best = 0
        var run = 0
        var previous: Int? = null
        for (year in years) {
            run = if (previous != null && year == previous + 1) run + 1 else 1
            best = maxOf(best, run)
            previous = year
        }
        return best
    }

    private const val MONTHS_IN_YEAR = 12
    private const val EXTRA_SCAN = 28
    private const val MAX_MONTH_WORDS = 3
    private const val YEAR_SCAN = 120
    private const val MIN_YEAR_RUN = 10
    private const val MIN_TWO_DIGIT_YEAR = 20
    private const val CENTURY = 2000
    private const val MIN_NAME_PREFIX = 3
    private val NUMBER_WITH_LETTERS = Regex("\\d+[a-z]+")
    private val MONTH_NUMBER = Regex("0?[1-9]|1[0-2]")
    private val YEAR = Regex("(?:20)?(\\d{2})")
}
