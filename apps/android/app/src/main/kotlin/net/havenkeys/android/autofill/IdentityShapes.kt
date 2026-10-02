package net.havenkeys.android.autofill

import java.time.LocalDate
import java.time.ZoneOffset

/** identity-fill.ts's text shapes for dates and phone numbers. */
internal object IdentityShapes {
    /** As saved, then without +55, when the field is shorter. */
    fun phone(f: FieldFacts, value: String): String? {
        val max = maxLengthOf(f)
        val national = value.replace(BRAZIL_PREFIX, "")
        return listOf(value, national).firstOrNull { max < 0 || it.length <= max }
    }

    /** An ISO birth date in the placeholder's format; null when it shows none. */
    fun birthDate(placeholder: String, iso: String): String? {
        val parts = isoDate(iso)?.let { iso.split('-') } ?: return null
        val (y, m, d) = parts
        return DATE_FORMATS.firstNotNullOfOrNull { (regex, order) ->
            regex.find(placeholder)?.let { found -> order(y, m, d).joinToString(found.groupValues[1]) }
        }
    }

    fun isoDate(value: String): LocalDate? =
        if (ISO_DATE.matches(value)) runCatching { LocalDate.parse(value) }.getOrNull() else null

    fun dateOf(day: LocalDate) = Shaped.Date(day.atStartOfDay(ZoneOffset.UTC).toInstant().toEpochMilli())

    private val ISO_DATE = Regex("\\d{4}-\\d{2}-\\d{2}")
    private val BRAZIL_PREFIX = Regex("^\\+55\\s*")
    private val DATE_FORMATS: List<Pair<Regex, (String, String, String) -> List<String>>> = listOf(
        Regex("(?:^|[^a-z])dd([/.-])mm\\1(?:aaaa|yyyy)(?![a-z])") to { y, m, d -> listOf(d, m, y) },
        Regex("(?:^|[^a-z])mm([/.-])dd\\1(?:aaaa|yyyy)(?![a-z])") to { y, m, d -> listOf(m, d, y) },
        Regex("(?:^|[^a-z])(?:aaaa|yyyy)([/.-])mm\\1dd(?![a-z])") to { y, m, d -> listOf(y, m, d) },
    )
}
