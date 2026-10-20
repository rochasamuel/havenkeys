package net.havenkeys.android.autofill

import android.text.InputType
import android.view.View

enum class FieldRole { USERNAME, PASSWORD, NEW_PASSWORD, CONFIRM_PASSWORD, OTP, UNKNOWN }

data class Classification(val role: FieldRole, val confidence: Int)

/**
 * Scores each role from several signals, never one (CLAUDE.md §21), like the
 * extension's engine (apps/extension/src/autofill/classify.ts, whose keyword
 * lists and weights this reuses; docs/autofill.md). Page and app strings are
 * only compared against the fixed lists below.
 */
object FieldClassifier {
    private val UNKNOWN = Classification(FieldRole.UNKNOWN, 0)

    fun classify(f: FieldFacts): Classification {
        if (!f.visible || !f.enabled || !isTextInput(f)) return UNKNOWN
        val s = Signals(f)
        val candidates = listOf(
            Triple(passwordRole(s), passwordScore(s), PASSWORD_THRESHOLD),
            Triple(FieldRole.USERNAME, usernameScore(s), USERNAME_THRESHOLD),
            Triple(FieldRole.OTP, otpScore(s), OTP_THRESHOLD),
        )
        val best = candidates.filter { it.second >= it.third }.maxByOrNull { it.second }
        // Checked only for a field that would be classified: a card field only ever gets cards.
        val isCard = best != null && CardFieldClassifier.kindOf(f, strong = false) != null
        return if (best == null || isCard) {
            UNKNOWN
        } else {
            Classification(best.first, best.second.coerceAtMost(MAX_CONFIDENCE))
        }
    }

    /** A visible, enabled text field that nothing rules out as a username. */
    internal fun couldBeUsername(f: FieldFacts): Boolean =
        isLoginCandidate(f) && usernameScore(Signals(f)) >= 0

    /** One box of a code split across one-character inputs (group.ts's split code field). */
    internal fun isCodeBox(f: FieldFacts): Boolean {
        val s = Signals(f)
        return isLoginCandidate(f) && !s.isPassword && s.maxLength == 1
    }

    /** An input that can hold text at all; any other HTML control, list or date is never filled. */
    internal fun isTextInput(f: FieldFacts): Boolean {
        val type = f.htmlAttributes["type"]?.lowercase()
        val isInput = f.htmlTag == null || f.htmlTag.equals("input", ignoreCase = true)
        return f.autofillType == View.AUTOFILL_TYPE_TEXT && isInput &&
            (type == null || type in TEXT_TYPES || type == "password")
    }

    private fun passwordScore(s: Signals): Int {
        var score = 0
        if (s.tokens.any { it in PASSWORD_HINTS }) score += HINT
        if ("newpassword" in s.tokens || (s.isPassword && "new-password" in s.tokens)) score += HINT
        if (s.isPassword) score += PASSWORD_INPUT
        if (s.attrs.has(PASSWORD_WORDS)) score += WORD_IN_ATTRS_WEAK
        if (s.text.has(PASSWORD_WORDS)) score += WORD_IN_TEXT_WEAK
        return score
    }

    /** Which kind of password field: explicit autocomplete first, then wording (classify.ts). */
    private fun passwordRole(s: Signals): FieldRole {
        val explicitNew = "newpassword" in s.tokens || (s.isPassword && "new-password" in s.tokens)
        return when {
            "current-password" in s.tokens || s.words.has(PW_CURRENT) -> FieldRole.PASSWORD
            s.words.has(PW_CONFIRM) && (explicitNew || s.isPassword) -> FieldRole.CONFIRM_PASSWORD
            explicitNew || (s.isPassword && s.words.has(PW_NEW)) -> FieldRole.NEW_PASSWORD
            else -> FieldRole.PASSWORD
        }
    }

    /** Negative when the field is ruled out as a username. */
    private fun usernameScore(s: Signals): Int = if (ruledOutAsUsername(s)) EXCLUDED else usernameEvidence(s)

    // `new-password` is not ruled out: sites put it on every field (gov.br's
    // CPF box, for one) to keep browsers' own autofill away.
    private fun ruledOutAsUsername(s: Signals) = s.isPassword || s.htmlType == "search" || s.isCard ||
        "one-time-code" in s.tokens || "current-password" in s.tokens || s.tokens.any { it in AC_NOT_LOGIN }

    private fun usernameEvidence(s: Signals): Int {
        var score = 0
        if (s.tokens.any { it in USERNAME_HINTS }) score += HINT
        if (s.tokens.any { it in PHONE_HINTS }) score += PHONE_HINT
        if (s.isEmail) score += EMAIL_INPUT
        score += when {
            s.attrs.has(USERNAME_STRONG) -> WORD_IN_ATTRS
            s.attrs.has(USERNAME_WEAK) -> WORD_IN_ATTRS_WEAK_USER
            else -> 0
        }
        score += when {
            s.text.has(USERNAME_STRONG) -> WORD_IN_TEXT
            s.text.has(USERNAME_WEAK) -> WORD_IN_TEXT_WEAK
            else -> 0
        }
        if (s.words.has(USERNAME_NEGATIVE)) score -= NEGATIVE
        return score
    }

    private fun otpScore(s: Signals): Int {
        if (s.isCard || s.words.has(OTP_NEGATIVE) || s.tokens.any { it in AC_NOT_LOGIN }) return 0
        var score = 0
        if ("one-time-code" in s.tokens || s.tokens.any { "otp" in it }) score += OTP_HINT
        score += when {
            s.words.has(OTP_STRONG) -> OTP_WORD
            s.words.has(OTP_WEAK) -> OTP_WORD_WEAK
            else -> 0
        }
        if (s.maxLength in OTP_MIN_LENGTH..OTP_MAX_LENGTH) score += OTP_LENGTH
        if (s.isNumeric) score += OTP_NUMERIC
        return score
    }

    /** One field's signals, normalized once. */
    private class Signals(f: FieldFacts) {
        val htmlType = f.htmlAttributes["type"]?.lowercase()

        /** Android autofill hints and HTML autocomplete tokens, lowercased. */
        val tokens: Set<String> = (
            f.autofillHints.map { it.lowercase() } +
                f.htmlAttributes["autocomplete"].orEmpty().lowercase().split(' ', '\t', '\n')
            ).filter { it.isNotEmpty() }.toSet()

        /** Identifiers: resource id, `name`, `id`. */
        val attrs = Words(f.words.attrs)

        /** What the user reads: hint, content description, placeholder, labels. */
        val text = Words(f.words.labels)
        val words = attrs + text

        private val inputClass = f.inputType and InputType.TYPE_MASK_CLASS
        private val variation = f.inputType and InputType.TYPE_MASK_VARIATION
        val isPassword = htmlType == "password" || isPasswordInputType(f.inputType)
        val isEmail = htmlType == "email" ||
            (inputClass == InputType.TYPE_CLASS_TEXT && variation in EMAIL_VARIATIONS)
        val isNumeric = inputClass == InputType.TYPE_CLASS_NUMBER || inputClass == InputType.TYPE_CLASS_PHONE ||
            htmlType == "tel" || htmlType == "number" || f.htmlAttributes["inputmode"]?.lowercase() == "numeric"
        val maxLength = maxLengthOf(f)
        val isCard = tokens.any { it.startsWith("cc-") } || attrs.has(CARD_WORDS)
    }

    /** Normalized text, matched against keyword lists as whole words (text.ts). */
    private class Words(val normalized: String) {
        private val padded = " $normalized "
        private val wordSet = if (normalized.isEmpty()) emptySet() else normalized.split(' ').toSet()

        fun has(phrases: Phrases) = wordSet.isNotEmpty() &&
            (wordSet.any { it in phrases.single } || phrases.multi.any { padded.contains(it) })

        operator fun plus(other: Words) = Words("$normalized ${other.normalized}".trim())
    }

    /** A keyword list: one-word phrases looked up, longer ones searched padded with spaces. */
    private class Phrases(all: List<String>) {
        val single = all.filter { ' ' !in it }.toSet()
        val multi = all.filter { ' ' in it }.map { " $it " }
    }

    private val EMAIL_VARIATIONS = setOf(
        InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS,
        InputType.TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS,
    )

    /** HTML input types that can hold a username or one-time code (classify.ts). */
    private val TEXT_TYPES = setOf("text", "email", "tel", "number", "search", "url", "")

    // Android View.AUTOFILL_HINT_* / androidx HintConstants values and HTML
    // autocomplete tokens, lowercased.
    private val USERNAME_HINTS = SharedWords.USERNAME_HINTS.toSet()
    private val PHONE_HINTS = SharedWords.PHONE_HINTS.toSet()
    private val PASSWORD_HINTS = SharedWords.PASSWORD_HINTS.toSet()

    // Keyword lists: packages/field-words/words.json (shared with the extension),
    // matched as whole normalized words.
    private val USERNAME_STRONG = Phrases(SharedWords.USERNAME_STRONG + SharedWords.USERNAME_DOCUMENT)
    private val USERNAME_WEAK = Phrases(SharedWords.USERNAME_WEAK)

    // Android's list has no "address": "email address" names a username.
    private val USERNAME_NEGATIVE = Phrases(SharedWords.USERNAME_NEGATIVE)
    private val AC_NOT_LOGIN = SharedWords.AC_NOT_LOGIN.toSet()
    private val CARD_WORDS = Phrases(SharedWords.LOGIN_CARD_WORDS)

    private val PASSWORD_WORDS = Phrases(SharedWords.PASSWORD_WORDS)
    private val PW_CONFIRM = Phrases(SharedWords.PW_CONFIRM)
    private val PW_NEW = Phrases(SharedWords.PW_NEW)
    private val PW_CURRENT = Phrases(SharedWords.PW_CURRENT)

    private val OTP_STRONG = Phrases(SharedWords.OTP_STRONG)
    private val OTP_WEAK = Phrases(SharedWords.OTP_WEAK)
    private val OTP_NEGATIVE = Phrases(SharedWords.OTP_NEGATIVE)
}

// Signal weights (packages/field-words/words.json; a role needs its threshold to win).
private const val USERNAME_THRESHOLD = SharedWords.USERNAME_THRESHOLD
private const val PASSWORD_THRESHOLD = SharedWords.PASSWORD_THRESHOLD
private const val OTP_THRESHOLD = SharedWords.OTP_THRESHOLD
private const val MAX_CONFIDENCE = 100
private const val EXCLUDED = -1000

private const val HINT = SharedWords.HINT
private const val PHONE_HINT = SharedWords.USERNAME_AC_TEL
private const val EMAIL_INPUT = SharedWords.USERNAME_EMAIL_INPUT
private const val PASSWORD_INPUT = SharedWords.PASSWORD_INPUT
private const val WORD_IN_ATTRS = SharedWords.USERNAME_WORD_IN_ATTRS
private const val WORD_IN_TEXT = SharedWords.USERNAME_WORD_IN_TEXT
private const val WORD_IN_ATTRS_WEAK_USER = SharedWords.USERNAME_WORD_IN_ATTRS_WEAK
private const val WORD_IN_ATTRS_WEAK = SharedWords.PASSWORD_WORD_IN_ATTRS_WEAK
private const val WORD_IN_TEXT_WEAK = SharedWords.USERNAME_WORD_IN_TEXT_WEAK
private const val NEGATIVE = SharedWords.USERNAME_NEGATIVE_PENALTY

private const val OTP_HINT = SharedWords.OTP_AC
private const val OTP_WORD = SharedWords.OTP_WORD
private const val OTP_WORD_WEAK = SharedWords.OTP_WORD_WEAK
private const val OTP_LENGTH = SharedWords.OTP_LENGTH
private const val OTP_NUMERIC = SharedWords.OTP_NUMERIC
private const val OTP_MIN_LENGTH = SharedWords.OTP_MIN_LENGTH
private const val OTP_MAX_LENGTH = SharedWords.OTP_MAX_LENGTH

/** A visible, enabled text input that is not a card field: a card field only ever gets cards. */
private fun isLoginCandidate(f: FieldFacts): Boolean =
    f.visible && f.enabled && FieldClassifier.isTextInput(f) && CardFieldClassifier.kindOf(f, strong = false) == null
