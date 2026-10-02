package net.havenkeys.android.autofill

import uniffi.havenkeys_mobile.IdentityRole

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

    /** The month a text names: a number 1–12 or a month name or its first letters. */
    fun monthNumber(text: String): Int? {
        val w = words(text)
        val number = w.firstNotNullOfOrNull { it.toIntOrNull() }
        val named = MONTHS.indexOfFirst { names -> w.any { it in names } }.takeIf { it >= 0 }?.plus(1)
        return (number ?: named)?.takeIf { it in 1..MONTHS_IN_YEAR }
    }

    /** The option for month [month] ("4"): "4", "04", "Abril", "abr", "04 - Abril". */
    fun month(options: List<String>, month: String): Int {
        val n = month.toIntOrNull() ?: return -1
        val wanted = setOf("$n", "$n".padStart(2, '0')) + MONTHS.getOrElse(n - 1) { emptyList() }
        return indexOf(options) { w -> w.isNotEmpty() && w.all { it in wanted } }
    }

    /** The option for [year] ("2033"): "2033" or "33". */
    fun year(options: List<String>, year: String): Int {
        val wanted = setOf(year, year.takeLast(2))
        return indexOf(options) { w -> w.size == 1 && w[0] in wanted }
    }

    /** The option naming brand [id] ("visa") by name or common code. */
    fun brand(options: List<String>, id: String): Int {
        val wanted = BRAND_ALIASES[id].orEmpty()
        return indexOf(options) { w -> w.joinToString(" ") in wanted }
    }

    /** The option for an identity value: as saved, or a Brazilian state's or Brazil's other names. */
    fun identity(options: List<String>, role: IdentityRole, value: String): Int {
        val v = normalize(value)
        val wanted = when (role) {
            IdentityRole.STATE -> UF.firstOrNull { (code, name) -> v == code || v == name }?.toList() ?: listOf(v)
            IdentityRole.COUNTRY -> if (v in BRAZIL) BRAZIL else listOf(v)
            else -> listOf(v)
        }
        return indexOf(options) { w -> w.joinToString(" ") in wanted }
    }

    private fun indexOf(options: List<String>, matches: (List<String>) -> Boolean): Int =
        options.take(MAX_SCAN).indexOfFirst { matches(words(it)) }

    const val MONTHS_IN_YEAR = 12
    private const val MAX_SCAN = 500
    private val BRAND_ALIASES = mapOf(
        "visa" to setOf("visa", "vi"),
        "mastercard" to setOf("mastercard", "master card", "master", "mc"),
        "amex" to setOf("amex", "american express", "ax"),
        "elo" to setOf("elo"),
        "hipercard" to setOf("hipercard", "hiper", "hc"),
        "diners" to setOf("diners", "diners club", "dc"),
        "discover" to setOf("discover", "di"),
        "jcb" to setOf("jcb"),
        "unionpay" to setOf("unionpay", "union pay", "cup"),
        "maestro" to setOf("maestro"),
    )
    private val UF: List<Pair<String, String>> = listOf(
        "ac" to "acre", "al" to "alagoas", "ap" to "amapa", "am" to "amazonas", "ba" to "bahia",
        "ce" to "ceara", "df" to "distrito federal", "es" to "espirito santo", "go" to "goias",
        "ma" to "maranhao", "mt" to "mato grosso", "ms" to "mato grosso do sul", "mg" to "minas gerais",
        "pa" to "para", "pb" to "paraiba", "pr" to "parana", "pe" to "pernambuco", "pi" to "piaui",
        "rj" to "rio de janeiro", "rn" to "rio grande do norte", "rs" to "rio grande do sul",
        "ro" to "rondonia", "rr" to "roraima", "sc" to "santa catarina", "sp" to "sao paulo",
        "se" to "sergipe", "to" to "tocantins",
    )
    private val BRAZIL = listOf("br", "bra", "brasil", "brazil")
}
