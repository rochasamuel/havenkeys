package net.havenkeys.android.autofill

import android.text.InputType
import java.text.Normalizer

/** Longest string read from any one source, as text.ts's MAX_HINT_CHARS. */
internal const val MAX_TEXT_CHARS = 200
private const val ASCII_END = 128
private const val MAX_DIGITS = 6

/**
 * `"loginEmail_Endereço"` → `"login email endereco"` (text.ts's normalize).
 * Page and app strings are only ever compared against fixed lists.
 */
internal fun normalize(raw: String?): String = raw
    ?.take(MAX_TEXT_CHARS)
    ?.replace(CAMEL, "\$1 \$2")
    ?.let { if (it.all { c -> c.code < ASCII_END }) it else stripAccents(it) }
    ?.lowercase()
    ?.replace(NOT_WORD, " ")
    ?.trim()
    .orEmpty()

/** Several sources normalized into one text. */
internal fun normalizeAll(vararg raw: String?): String =
    raw.map(::normalize).filter { it.isNotEmpty() }.joinToString(" ")

/** Does normalized [text] contain any of [phrases] as whole words? */
internal fun hasAny(text: String, phrases: Collection<String>): Boolean {
    val padded = " $text "
    return text.isNotEmpty() && phrases.any { padded.contains(" $it ") }
}

internal fun isPasswordInputType(inputType: Int): Boolean {
    val inputClass = inputType and InputType.TYPE_MASK_CLASS
    val variation = inputType and InputType.TYPE_MASK_VARIATION
    return (inputClass == InputType.TYPE_CLASS_TEXT && variation in TEXT_PASSWORDS) ||
        (inputClass == InputType.TYPE_CLASS_NUMBER && variation == InputType.TYPE_NUMBER_VARIATION_PASSWORD)
}

/** The HTML `maxlength`, else the native limit; -1 when unknown. */
internal fun maxLengthOf(f: FieldFacts): Int =
    f.htmlAttributes["maxlength"]?.trim()?.take(MAX_DIGITS)?.toIntOrNull()?.takeIf { it > 0 } ?: f.maxTextLength

private val TEXT_PASSWORDS = setOf(
    InputType.TYPE_TEXT_VARIATION_PASSWORD,
    InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD,
    InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD,
)

private fun stripAccents(text: String) =
    Normalizer.normalize(text, Normalizer.Form.NFKD).replace(COMBINING_MARKS, "")

private val CAMEL = Regex("([a-z])([A-Z])")
private val COMBINING_MARKS = Regex("\\p{Mn}+")
private val NOT_WORD = Regex("[^a-z0-9]+")
