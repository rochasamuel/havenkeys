package net.havenkeys.android.autofill

import android.text.InputType
import android.view.View

/** A Brazilian bank sign-in's boxes: agência, conta and their check digits. */
enum class BankRole { BRANCH, BRANCH_DIGIT, ACCOUNT, ACCOUNT_DIGIT }

/** A bank box of a form, by [FieldFacts.index]. */
data class BankBox(val index: Int, val role: BankRole)

/** What a box's group says around it: another box with a branch or account word, and the bank role just before it. */
data class BankGroupFacts(
    val groupHasBranch: Boolean = false,
    val groupHasAccount: Boolean = false,
    val prevRole: BankRole? = null,
    /** The next box is a one-character box too: a split one-time code, not a check digit. */
    val nextIsSingle: Boolean = false,
)

/**
 * The extension's bank.ts (spec 2026-11-03-bank-account §5.1). "Conta" alone
 * means nothing ("Entrar na sua conta"): it counts only beside a branch box.
 * The login and identity finders refuse whatever this claims. Pure reads
 * compared against fixed lists. Android shows no forms: a group is a frame.
 */
object BankFieldClassifier {
    fun roleOf(f: FieldFacts, g: BankGroupFacts): BankRole? {
        val words = wordsOf(f) ?: return null
        // Rule 1: a branch box looks numeric, or sits beside an account box.
        val numeric = isNumeric(f) || maxLengthOf(f) in 1..MAX_BRANCH_LENGTH
        // Rule 2: an account box only ever sits beside a branch box.
        return digitRole(f, g, words) ?: when {
            hasAny(words, SharedWords.BANK_BRANCH) && (numeric || g.groupHasAccount) -> BankRole.BRANCH
            hasAny(words, SharedWords.BANK_ACCOUNT) && g.groupHasBranch -> BankRole.ACCOUNT
            else -> null
        }
    }

    /**
     * Rule 3: a check digit right after a branch or account box. A digit word
     * counts on a box with no length limit; a longer box ("Conta com dígito",
     * maxlength 8) is the number itself. Not the first box of a split one-time code.
     */
    private fun digitRole(f: FieldFacts, g: BankGroupFacts, words: String): BankRole? {
        val max = maxLengthOf(f)
        val digit = when (g.prevRole) {
            BankRole.BRANCH -> BankRole.BRANCH_DIGIT
            BankRole.ACCOUNT -> BankRole.ACCOUNT_DIGIT
            else -> return null
        }
        val splitCode = max == 1 && g.nextIsSingle
        val shaped = max in 1..2 || (max < 0 && hasAny(words, SharedWords.BANK_DIGIT))
        return digit.takeIf { shaped && !splitCode && otpScore(f, words) < SharedWords.OTP_THRESHOLD }
    }

    /** The bank role of each box, by [FieldFacts.index], its facts read per frame in screen order. */
    fun roles(fields: List<FieldFacts>): Map<Int, BankRole> {
        val out = HashMap<Int, BankRole>()
        // The inputs the login finder reads, so the two never disagree.
        val usable = fields.filter { isUsable(it) }
        for (boxes in usable.groupBy { it.webDomain to it.webScheme }.values) {
            val words = boxes.map(::wordsOf)
            fun says(i: Int, list: List<String>) = words[i]?.let { hasAny(it, list) } == true
            val branches = boxes.indices.count { says(it, SharedWords.BANK_BRANCH) }
            val accounts = boxes.indices.count { says(it, SharedWords.BANK_ACCOUNT) }
            var prevRole: BankRole? = null
            boxes.forEachIndexed { i, f ->
                val g = BankGroupFacts(
                    groupHasBranch = branches - (if (says(i, SharedWords.BANK_BRANCH)) 1 else 0) > 0,
                    groupHasAccount = accounts - (if (says(i, SharedWords.BANK_ACCOUNT)) 1 else 0) > 0,
                    prevRole = prevRole,
                    nextIsSingle = boxes.getOrNull(i + 1)?.let(::maxLengthOf) == 1,
                )
                prevRole = roleOf(f, g)?.also { out[f.index] = it }
            }
        }
        return out
    }

    /** The box's normalized words, or null when it can never be a bank box (rule 4, BANK_NEGATIVE). */
    private fun wordsOf(f: FieldFacts): String? {
        val type = f.htmlAttributes["type"]?.lowercase()
        val refused = !f.visible || !f.enabled || "readonly" in f.htmlAttributes ||
            f.autofillType != View.AUTOFILL_TYPE_TEXT ||
            (f.htmlTag != null && !f.htmlTag.equals("input", ignoreCase = true)) ||
            (type != null && type !in TYPES) || isPasswordInputType(f.inputType) || isNativeEmail(f) ||
            namesAField(f) || CardFieldClassifier.kindOf(f, strong = false) != null
        return f.words.all.takeUnless { refused || hasAny(it, SharedWords.BANK_NEGATIVE) }
    }

    /** An autocomplete token or Android hint that names another field. Junk ("nope", "chrome-off") names nothing. */
    private fun namesAField(f: FieldFacts): Boolean {
        val tokens = f.htmlAttributes["autocomplete"].orEmpty().lowercase().split(' ', '\t', '\n')
        val hints = f.autofillHints.map { it.lowercase() }
        return tokens.any { AC_REAL.matches(it) } ||
            hints.any { AC_REAL.matches(it) || it in ANDROID_HINTS || it.startsWith("creditcard") }
    }

    /** bank.ts's isFillable: a readonly box is left out, so it never breaks a conta–dígito pair. */
    private fun isUsable(f: FieldFacts) = f.visible && f.enabled && "readonly" !in f.htmlAttributes &&
        FieldClassifier.isTextInput(f) && CardFieldClassifier.kindOf(f, strong = false) == null

    private fun isNumeric(f: FieldFacts): Boolean {
        val type = f.htmlAttributes["type"]?.lowercase()
        val inputClass = f.inputType and InputType.TYPE_MASK_CLASS
        return type == "number" || type == "tel" || f.htmlAttributes["inputmode"]?.lowercase() == "numeric" ||
            (f.htmlTag == null && inputClass in NATIVE_NUMERIC)
    }

    private fun isNativeEmail(f: FieldFacts): Boolean {
        val variation = f.inputType and InputType.TYPE_MASK_VARIATION
        return f.htmlTag == null && (f.inputType and InputType.TYPE_MASK_CLASS) == InputType.TYPE_CLASS_TEXT &&
            (variation == InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS ||
                variation == InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS)
    }

    /** classify.ts's otpScore as bank.ts calls it: no autocomplete, no mask, no split group. */
    private fun otpScore(f: FieldFacts, words: String): Int {
        if (hasAny(words, SharedWords.LOGIN_CARD_WORDS) || hasAny(words, SharedWords.OTP_NEGATIVE)) return 0
        var score = when {
            hasAny(words, SharedWords.OTP_STRONG) -> SharedWords.OTP_WORD
            hasAny(words, SharedWords.OTP_WEAK) -> SharedWords.OTP_WORD_WEAK
            else -> 0
        }
        if (maxLengthOf(f) in SharedWords.OTP_MIN_LENGTH..SharedWords.OTP_MAX_LENGTH) score += SharedWords.OTP_LENGTH
        if (isNumeric(f)) score += SharedWords.OTP_NUMERIC
        return score
    }

    private const val MAX_BRANCH_LENGTH = 8
    private val NATIVE_NUMERIC = setOf(InputType.TYPE_CLASS_NUMBER, InputType.TYPE_CLASS_PHONE)
    private val TYPES = setOf("", "text", "tel", "number")

    /** The autocomplete names HTML defines for a field that is not a bank box (bank.ts's AC_REAL). */
    private val AC_REAL = Regex(
        "username|email|impp|tel(-\\S+)?|cc-\\S+|one-time-code|(current|new)-password|name|honorific-\\S+|" +
            "given-name|additional-name|family-name|nickname|organization(-title)?|street-address|" +
            "address-line\\d|address-level\\d|postal-code|country(-name)?|bday(-\\S+)?|sex|url|photo|" +
            "transaction-\\S+|webauthn",
    )

    // Android's View.AUTOFILL_HINT_* and androidx HintConstants not already above, lowercased.
    private val ANDROID_HINTS = setOf(
        "emailaddress", "password", "phone", "phonenumber", "phonenational", "postaladdress", "postalcode",
        "newpassword", "newusername", "smsotpcode", "personname",
    )
}
