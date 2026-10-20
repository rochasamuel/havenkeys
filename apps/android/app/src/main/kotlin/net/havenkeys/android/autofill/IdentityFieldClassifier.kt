package net.havenkeys.android.autofill

import android.text.InputType
import android.view.View
import uniffi.havenkeys_mobile.IdentityRole

/** [byHint]: named by an Android autofill hint or an HTML autocomplete token. */
data class IdentityRoleOf(val role: IdentityRole, val byHint: Boolean)

/** Document numbers. Rust's FillRole::is_document names the same four and enforces them. */
val IdentityRole.isDocument: Boolean
    get() = this in DOCUMENT_ROLES

private val DOCUMENT_ROLES =
    setOf(IdentityRole.CPF, IdentityRole.RG, IdentityRole.PASSPORT, IdentityRole.DRIVERS_LICENSE)

/**
 * One field's identity role: the extension's identity.ts plus Android's
 * autofill hints. Pure reads compared against fixed lists. Card fields,
 * passwords and one-time codes are never identity fields.
 */
object IdentityFieldClassifier {
    fun roleOf(f: FieldFacts): IdentityRoleOf? {
        val shape = shapeOf(f)
        val s = IdentitySignals(f)
        val isCard = shape != Shape.MULTI_LINE && CardFieldClassifier.kindOf(f, false) != null
        val refused = shape == null || s.refused || isCard
        // Sites use type=tel for any digits (birth date, CEP, CPF): the words
        // decide before a tel box is taken for a phone.
        val found = if (refused) null else byHint(s) ?: byEmailType(f) ?: byWords(f, s) ?: byPhoneType(f)
        return found?.let { adjust(it, shape) }
    }

    /** What a field can hold: a date only a birth date, a multi-line box only an address. */
    private enum class Shape { TEXT, LIST, DATE, MULTI_LINE }

    private fun adjust(found: IdentityRoleOf, shape: Shape?): IdentityRoleOf? = when (shape) {
        Shape.DATE -> found.takeIf { it.role == IdentityRole.BIRTH_DATE }
        Shape.MULTI_LINE -> found.takeIf { it.role in ADDRESS_LINES }
            ?.let { it.copy(role = IdentityRole.ADDRESS_LINE1) }
        else -> found
    }

    private fun shapeOf(f: FieldFacts): Shape? {
        val tag = f.htmlTag?.lowercase()
        val type = f.htmlAttributes["type"]?.lowercase()
        val multiLine = tag == "textarea" ||
            (tag == null && f.inputType and InputType.TYPE_TEXT_FLAG_MULTI_LINE != 0)
        return when {
            f.isList || tag == "select" -> Shape.LIST
            f.isDate || type == "date" -> Shape.DATE
            f.autofillType != View.AUTOFILL_TYPE_TEXT -> null
            type == "password" || isPasswordInputType(f.inputType) -> null
            multiLine -> Shape.MULTI_LINE
            else -> textShape(tag, type)
        }
    }

    private fun textShape(tag: String?, type: String?): Shape? = when {
        tag != null && tag != "input" -> null
        type == null || type in TEXT_TYPES -> Shape.TEXT
        else -> null
    }

    private class IdentitySignals(f: FieldFacts) {
        val hints = f.autofillHints.map { it.lowercase() }
        private val rawAutocomplete = f.htmlAttributes["autocomplete"].orEmpty().lowercase()
        val autocomplete = rawAutocomplete.split(' ', '\t', '\n').filter { it.isNotEmpty() && !AC_PREFIX.matches(it) }
        val attrs = f.words.attrs
        val text = f.words.text
        val all = f.words.all

        // Checkouts put new-password on address fields to keep browsers away:
        // only card and one-time-code tokens refuse a text field here.
        val refused = REFUSED_TOKEN.containsMatchIn(rawAutocomplete) ||
            hints.any { it.startsWith("creditcard") || it == "smsotpcode" } ||
            hasAny(all, NEGATIVE)
    }

    private fun byHint(s: IdentitySignals): IdentityRoleOf? {
        val role = s.hints.firstNotNullOfOrNull { HINTS[it] ?: AUTOCOMPLETE[it] }
            ?: s.autocomplete.lastOrNull()?.let { AUTOCOMPLETE[it] }
        return role?.let { IdentityRoleOf(it, byHint = true) }
    }

    private fun byEmailType(f: FieldFacts): IdentityRoleOf? {
        val inputClass = f.inputType and InputType.TYPE_MASK_CLASS
        val variation = f.inputType and InputType.TYPE_MASK_VARIATION
        val isEmail = f.htmlAttributes["type"]?.lowercase() == "email" ||
            (inputClass == InputType.TYPE_CLASS_TEXT && variation in EMAIL_VARIATIONS)
        return if (isEmail) IdentityRoleOf(IdentityRole.EMAIL, byHint = false) else null
    }

    private fun byPhoneType(f: FieldFacts): IdentityRoleOf? {
        val isPhone = f.htmlAttributes["type"]?.lowercase() == "tel" ||
            (f.inputType and InputType.TYPE_MASK_CLASS) == InputType.TYPE_CLASS_PHONE
        return if (isPhone) IdentityRoleOf(IdentityRole.PHONE, byHint = false) else null
    }

    private fun byWords(f: FieldFacts, s: IdentitySignals): IdentityRoleOf? {
        val hit = WORDS.firstOrNull { (_, words) -> hasAny(s.attrs, words) || hasAny(s.text, words) }
        val role = hit?.let { (role, words) -> compound(labelledPart(role, words, s.text), s.all) }
        // "Número para contato" on a tel box is the phone; a house number names the address.
        val telNumber = role == IdentityRole.NUMBER && byPhoneType(f) != null && !hasAny(s.all, ADDRESS)
        return role?.let { IdentityRoleOf(if (telNumber) IdentityRole.PHONE else it, byHint = false) }
    }

    /**
     * A generic address in the name (`enderecoCidade`, `address[city]`) is
     * read as the street; the visible label naming a specific part (Cidade,
     * Estado, País…) wins over it (identity.ts's labelledPart).
     */
    private fun labelledPart(found: IdentityRole, words: List<String>, text: String): IdentityRole {
        if (found != IdentityRole.STREET || hasAny(text, words)) return found
        return WORDS.firstOrNull { (role, w) -> role in SPECIFIC_ADDRESS_ROLES && hasAny(text, w) }?.first ?: found
    }

    /** Null when the words make a role match wrong (identity.ts's compound labels). */
    private fun compound(role: IdentityRole, all: String): IdentityRole? = when {
        // "Cidade de nascimento", "Country of birth": a birthplace.
        hasAny(all, BIRTH) && hasAny(all, PLACE) -> null
        // "Número do documento": not a house number.
        role == IdentityRole.NUMBER && hasAny(all, DOCUMENT) -> null
        // "Endereço da empresa": the company's address, ambiguous either way.
        role == IdentityRole.COMPANY && hasAny(all, ADDRESS) -> null
        else -> role
    }

    private val TEXT_TYPES = setOf("text", "email", "tel", "number", "url", "")
    private val ADDRESS_LINES = setOf(IdentityRole.STREET, IdentityRole.ADDRESS_LINE1)
    private val EMAIL_VARIATIONS = setOf(
        InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
        InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS,
    )
    private val AC_PREFIX = Regex("section-\\S+|shipping|billing|home|work|mobile|fax|pager")
    private val REFUSED_TOKEN = Regex("\\b(cc-|one-time-code)")

    // View.AUTOFILL_HINT_* and androidx HintConstants, lowercased.
    private val HINTS = mapOf(
        "personname" to IdentityRole.FULL_NAME,
        "persongivenname" to IdentityRole.FIRST_NAME,
        "personmiddlename" to IdentityRole.MIDDLE_NAME,
        "personfamilyname" to IdentityRole.LAST_NAME,
        "emailaddress" to IdentityRole.EMAIL,
        "phone" to IdentityRole.PHONE,
        "phonenumber" to IdentityRole.PHONE,
        "phonenational" to IdentityRole.PHONE,
        "postalcode" to IdentityRole.POSTAL_CODE,
        "streetaddress" to IdentityRole.ADDRESS_LINE1,
        "extendedaddress" to IdentityRole.ADDRESS_LINE2,
        "addresslocality" to IdentityRole.CITY,
        "addressregion" to IdentityRole.STATE,
        "addresscountry" to IdentityRole.COUNTRY,
        "birthdatefull" to IdentityRole.BIRTH_DATE,
        "birthdateday" to IdentityRole.BIRTH_DAY,
        "birthdatemonth" to IdentityRole.BIRTH_MONTH,
        "birthdateyear" to IdentityRole.BIRTH_YEAR,
        "username" to IdentityRole.USERNAME,
        "newusername" to IdentityRole.USERNAME,
    )

    // identity.ts's AUTOCOMPLETE.
    private val AUTOCOMPLETE = mapOf(
        "name" to IdentityRole.FULL_NAME,
        "given-name" to IdentityRole.FIRST_NAME,
        "additional-name" to IdentityRole.MIDDLE_NAME,
        "family-name" to IdentityRole.LAST_NAME,
        "email" to IdentityRole.EMAIL,
        "tel" to IdentityRole.PHONE,
        "tel-national" to IdentityRole.PHONE,
        "bday" to IdentityRole.BIRTH_DATE,
        "bday-day" to IdentityRole.BIRTH_DAY,
        "bday-month" to IdentityRole.BIRTH_MONTH,
        "bday-year" to IdentityRole.BIRTH_YEAR,
        "organization" to IdentityRole.COMPANY,
        "street-address" to IdentityRole.ADDRESS_LINE1,
        "address-line1" to IdentityRole.ADDRESS_LINE1,
        "address-line2" to IdentityRole.ADDRESS_LINE2,
        "address-level3" to IdentityRole.NEIGHBORHOOD,
        "address-level2" to IdentityRole.CITY,
        "address-level1" to IdentityRole.STATE,
        "postal-code" to IdentityRole.POSTAL_CODE,
        "country" to IdentityRole.COUNTRY,
        "country-name" to IdentityRole.COUNTRY,
        "username" to IdentityRole.USERNAME,
    )

    // identity.ts's WORDS order (lists in words.json): the first hit wins.
    private val WORDS: List<Pair<IdentityRole, List<String>>> = listOf(
        IdentityRole.FULL_NAME to SharedWords.IDENTITY_WORDS_FULL_NAME,
        IdentityRole.FIRST_NAME to SharedWords.IDENTITY_WORDS_FIRST_NAME,
        IdentityRole.MIDDLE_NAME to SharedWords.IDENTITY_WORDS_MIDDLE_NAME,
        IdentityRole.LAST_NAME to SharedWords.IDENTITY_WORDS_LAST_NAME,
        IdentityRole.CPF to SharedWords.IDENTITY_WORDS_CPF,
        IdentityRole.RG to SharedWords.IDENTITY_WORDS_RG,
        IdentityRole.PASSPORT to SharedWords.IDENTITY_WORDS_PASSPORT,
        IdentityRole.DRIVERS_LICENSE to SharedWords.IDENTITY_WORDS_DRIVERS_LICENSE,
        IdentityRole.BIRTH_DATE to SharedWords.IDENTITY_WORDS_BIRTH_DATE,
        IdentityRole.EMAIL to SharedWords.IDENTITY_WORDS_EMAIL,
        IdentityRole.PHONE to SharedWords.IDENTITY_WORDS_PHONE,
        IdentityRole.USERNAME to SharedWords.IDENTITY_WORDS_USERNAME,
        IdentityRole.COMPANY to SharedWords.IDENTITY_WORDS_COMPANY,
        IdentityRole.POSTAL_CODE to SharedWords.IDENTITY_WORDS_POSTAL_CODE,
        IdentityRole.NEIGHBORHOOD to SharedWords.IDENTITY_WORDS_NEIGHBORHOOD,
        IdentityRole.COMPLEMENT to SharedWords.IDENTITY_WORDS_COMPLEMENT,
        IdentityRole.NUMBER to SharedWords.IDENTITY_WORDS_NUMBER,
        IdentityRole.STREET to SharedWords.IDENTITY_WORDS_STREET,
        IdentityRole.CITY to SharedWords.IDENTITY_WORDS_CITY,
        IdentityRole.STATE to SharedWords.IDENTITY_WORDS_STATE,
        IdentityRole.COUNTRY to SharedWords.IDENTITY_WORDS_COUNTRY,
        IdentityRole.FULL_NAME to SharedWords.IDENTITY_WORDS_FULL_NAME_GENERIC,
    )
    private val NEGATIVE = SharedWords.IDENTITY_NEGATIVE
    private val SPECIFIC_ADDRESS_ROLES = SharedWords.IDENTITY_SPECIFIC_ADDRESS_ROLES
        .map { IdentityRole.valueOf(it.replace(Regex("([a-z])([A-Z])"), "$1_$2").uppercase()) }.toSet()
    private val BIRTH = SharedWords.IDENTITY_BIRTH
    private val PLACE = SharedWords.IDENTITY_PLACE
    private val DOCUMENT = SharedWords.IDENTITY_DOCUMENT
    private val ADDRESS = SharedWords.IDENTITY_ADDRESS
}
