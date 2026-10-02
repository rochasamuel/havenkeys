package net.havenkeys.android.autofill

import android.view.View

/** What a card field asks for; [EXPIRY] is month and year in one field. */
enum class CardKind {
    CARDHOLDER_NAME, CARDHOLDER_GIVEN_NAME, CARDHOLDER_FAMILY_NAME, NUMBER, VERIFICATION_NUMBER,
    EXPIRY, EXPIRY_MONTH, EXPIRY_YEAR, BRAND,
}

/** [byHint]: named by an Android autofill hint or an HTML `cc-*` token. */
data class CardKindOf(val kind: CardKind, val byHint: Boolean)

/**
 * One field's card kind: the extension's card-kind.ts plus Android's
 * autofill hints. Pure reads compared against fixed lists. The login and
 * identity classifiers refuse whatever this claims, so a card field only
 * ever gets cards.
 */
object CardFieldClassifier {
    /** [strong]: the field's frame already has a number, code or expiry field. */
    fun kindOf(f: FieldFacts, strong: Boolean): CardKindOf? {
        val shape = shapeOf(f) ?: return null
        val s = CardSignals(f)
        // Refusals are checked only once something matched: most fields match nothing.
        val hit = hintHit(s) ?: wordHit(s, strong, shape) ?: weakHit(f, s, shape, strong)
        val of = (hit as? Hit.Of)?.takeIf { shape.allows(it.kind) && !s.refused }
        return of?.let { CardKindOf(it.kind, it.byHint) }
    }

    private sealed interface Hit {
        data class Of(val kind: CardKind, val byHint: Boolean = false) : Hit
        data object Refused : Hit
    }

    /** What a field can hold. A password box only ever holds the code; a list a month, year or brand. */
    private enum class Shape {
        TEXT, SECRET, LIST, DATE;

        fun allows(kind: CardKind) = when (this) {
            TEXT -> true
            SECRET -> kind == CardKind.VERIFICATION_NUMBER
            LIST -> kind in LIST_KINDS
            DATE -> kind == CardKind.EXPIRY
        }
    }

    private fun shapeOf(f: FieldFacts): Shape? = when {
        f.isList || f.htmlTag.equals("select", ignoreCase = true) -> Shape.LIST
        f.isDate -> Shape.DATE
        f.autofillType != View.AUTOFILL_TYPE_TEXT -> null
        f.htmlTag != null && !f.htmlTag.equals("input", ignoreCase = true) -> null
        else -> textShape(f)
    }

    private fun textShape(f: FieldFacts): Shape? {
        val type = f.htmlAttributes["type"]?.lowercase()
        return when {
            type == "password" || isPasswordInputType(f.inputType) -> Shape.SECRET
            type == null || type in TEXT_TYPES -> Shape.TEXT
            else -> null
        }
    }

    private class CardSignals(f: FieldFacts) {
        val hints = f.autofillHints.map { it.lowercase() }
        val autocomplete = f.htmlAttributes["autocomplete"].orEmpty().lowercase()
            .split(' ', '\t', '\n').filter { it.isNotEmpty() && !AC_PREFIX.matches(it) }
        val attrs = normalizeAll(f.idEntry, f.htmlAttributes["name"], f.htmlAttributes["id"])
        val text = if (f.hint == null && f.contentDescription == null && f.htmlAttributes.isEmpty()) {
            ""
        } else {
            normalizeAll(
                f.hint, f.contentDescription, f.htmlAttributes["placeholder"],
                f.htmlAttributes["aria-label"], f.htmlAttributes["title"], f.htmlAttributes["label"],
            )
        }
        val all = "$attrs $text".trim()
        val refused: Boolean get() =
            hasAny(all, NEGATIVE) || "one-time-code" in autocomplete || "one-time-code" in hints
    }

    private fun hintHit(s: CardSignals): Hit? {
        val kind = s.hints.firstNotNullOfOrNull { HINTS[it] ?: AUTOCOMPLETE[it] }
            ?: s.autocomplete.lastOrNull()?.let { AUTOCOMPLETE[it] }
        return kind?.let { Hit.Of(it, byHint = true) }
    }

    private fun wordHit(s: CardSignals, strong: Boolean, shape: Shape): Hit? {
        // A password box only ever holds the code, which is the first list.
        val lists = if (shape == Shape.SECRET) WORDS.take(1) else WORDS
        val kind = lists.firstOrNull { (_, words) ->
            val usable = if (strong) words else words.filter { it !in AMBIGUOUS }
            hasAny(s.attrs, usable) || hasAny(s.text, usable)
        }?.first
        val refused = (kind == CardKind.CARDHOLDER_NAME && hasAny(s.all, NOT_A_NAME)) ||
            (kind == CardKind.EXPIRY && hasAny(s.all, NOT_AN_EXPIRY))
        return when {
            kind == null -> null
            refused -> Hit.Refused
            else -> Hit.Of(kind)
        }
    }

    /** Weak signals, only in a frame that already has a number, code or expiry field. */
    private fun weakHit(f: FieldFacts, s: CardSignals, shape: Shape, strong: Boolean): Hit? {
        val kind = when {
            !strong -> null
            shape == Shape.LIST && OptionLists.isMonthList(f.options) -> CardKind.EXPIRY_MONTH
            shape == Shape.LIST && OptionLists.isYearList(f.options) -> CardKind.EXPIRY_YEAR
            hasAny(s.all, MONTH_WORDS) -> CardKind.EXPIRY_MONTH
            hasAny(s.all, YEAR_WORDS) -> CardKind.EXPIRY_YEAR
            shape != Shape.LIST && maxLengthOf(f) in CODE_LENGTHS && hasAny(s.all, CODE_WORDS) ->
                CardKind.VERIFICATION_NUMBER
            else -> null
        }
        return kind?.let { Hit.Of(it) }
    }

    private val LIST_KINDS = setOf(CardKind.EXPIRY_MONTH, CardKind.EXPIRY_YEAR, CardKind.BRAND)

    /** HTML input types that can hold a card value. */
    private val TEXT_TYPES = setOf("text", "tel", "number", "")
    private val AC_PREFIX = Regex("section-\\S+|shipping|billing")
    private const val MIN_CODE_LENGTH = 3
    private const val MAX_CODE_LENGTH = 4
    private val CODE_LENGTHS = MIN_CODE_LENGTH..MAX_CODE_LENGTH

    // View.AUTOFILL_HINT_CREDIT_CARD_* and androidx HintConstants, lowercased.
    private val HINTS = mapOf(
        "creditcardnumber" to CardKind.NUMBER,
        "creditcardsecuritycode" to CardKind.VERIFICATION_NUMBER,
        "creditcardexpirationdate" to CardKind.EXPIRY,
        "creditcardexpirationmonth" to CardKind.EXPIRY_MONTH,
        "creditcardexpirationyear" to CardKind.EXPIRY_YEAR,
    )
    private val AUTOCOMPLETE = mapOf(
        "cc-name" to CardKind.CARDHOLDER_NAME,
        "cc-given-name" to CardKind.CARDHOLDER_GIVEN_NAME,
        "cc-family-name" to CardKind.CARDHOLDER_FAMILY_NAME,
        "cc-number" to CardKind.NUMBER,
        "cc-csc" to CardKind.VERIFICATION_NUMBER,
        "cc-exp" to CardKind.EXPIRY,
        "cc-exp-month" to CardKind.EXPIRY_MONTH,
        "cc-exp-year" to CardKind.EXPIRY_YEAR,
        "cc-type" to CardKind.BRAND,
    )

    // card-kind.ts's lists, in its order: the first hit wins.
    private val WORDS: List<Pair<CardKind, List<String>>> = listOf(
        CardKind.VERIFICATION_NUMBER to listOf(
            "cvv", "cvc", "csc", "cvv2", "cvc2", "cid", "security code", "codigo de seguranca",
            "cod seguranca", "card code", "card verification",
        ),
        CardKind.CARDHOLDER_NAME to listOf(
            "name on card", "nome impresso", "nome impresso no cartao", "nome no cartao", "nome do titular",
            "titular", "cardholder", "card holder", "holder name",
        ),
        CardKind.NUMBER to listOf(
            "card number", "numero do cartao", "numero cartao", "cardnumber", "ccnumber", "cc number",
            "credit card", "cartao de credito", "card no",
        ),
        CardKind.EXPIRY_MONTH to listOf(
            "exp month", "expiry month", "expiration month", "mes de validade", "mes validade", "mes de vencimento",
        ),
        CardKind.EXPIRY_YEAR to listOf(
            "exp year", "expiry year", "expiration year", "ano de validade", "ano validade", "ano de vencimento",
        ),
        CardKind.EXPIRY to listOf(
            "expiry", "expiration", "exp date", "expiry date", "expiration date", "valid thru", "validade",
            "vencimento", "data de validade", "mm aa", "mm yy",
        ),
        CardKind.BRAND to listOf("card type", "bandeira", "card brand"),
    )
    private val AMBIGUOUS = setOf(
        "cid", "security code", "codigo de seguranca", "cod seguranca", "card code", "titular", "cardholder",
        "card holder", "holder name", "nome do titular",
    )
    private val NOT_AN_EXPIRY = listOf("dd", "dia", "day", "nascimento", "birth", "bday", "birthday")
    private val MONTH_WORDS = listOf("month", "mes", "mm")
    private val YEAR_WORDS = listOf("year", "ano", "yy", "yyyy", "aa", "aaaa")
    private val CODE_WORDS = listOf("code", "codigo", "cod")
    private val NEGATIVE = listOf(
        "gift card", "cartao presente", "vale presente", "loyalty", "fidelidade", "coupon", "cupom", "promo",
        "search", "busca", "pesquisar", "parcela", "parcelas", "installment", "installments", "vezes", "quantidade",
    )
    private val NOT_A_NAME = listOf("cpf", "documento", "nascimento", "birth", "email", "e mail", "telefone", "phone")
}
