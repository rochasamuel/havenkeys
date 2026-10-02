package net.havenkeys.android.autofill

import uniffi.havenkeys_mobile.CardRole

/** One card field; [slice]: the digits of the number a split box takes. */
data class CardField(val index: Int, val kind: CardKind, val byHint: Boolean, val slice: IntRange? = null)

/** The card fields of one frame, in screen order. */
data class CardFrame(val webDomain: String?, val webScheme: String?, val fields: List<CardField>)

/** [frames]: the focused field's frame first (unfocused: the first qualifying one). */
data class CardForm(val frames: List<CardFrame>)

/**
 * A screen's card fields, grouped by frame (card.ts). Only the first frame
 * must qualify; the others add their card fields, so a processor's
 * one-field iframes count together. Rust decides which frames are filled.
 */
object CardFormFinder {
    /** Rust's MAX_CARD_FRAMES. */
    const val MAX_CARD_FRAMES = 8

    /** [useFocus] false: a confirmed save, read whatever is focused. */
    fun find(fields: List<FieldFacts>, useFocus: Boolean = true): CardForm? {
        val frames = fields.filter { it.visible && it.enabled }
            .groupBy { it.webDomain to it.webScheme }
            .map { (frame, inFrame) -> CardFrame(frame.first, frame.second, classify(inFrame)) }
            .filter { it.fields.isNotEmpty() }
        val focused = fields.firstOrNull { it.focused }?.index?.takeIf { useFocus }
        val first = if (focused != null) {
            frames.firstOrNull { frame -> frame.fields.any { it.index == focused } }
        } else {
            frames.firstOrNull(::qualifies)
        }
        return first?.takeIf(::qualifies)?.let { CardForm((listOf(it) + (frames - it)).take(MAX_CARD_FRAMES)) }
    }

    /** The roles a frame asks Rust for, once each, in screen order. */
    fun rolesOf(frame: CardFrame): List<CardRole> = frame.fields.flatMap { roles(it.kind) }.distinct()

    private fun roles(kind: CardKind): List<CardRole> = when (kind) {
        CardKind.CARDHOLDER_NAME -> listOf(CardRole.CARDHOLDER_NAME)
        CardKind.CARDHOLDER_GIVEN_NAME -> listOf(CardRole.CARDHOLDER_GIVEN_NAME)
        CardKind.CARDHOLDER_FAMILY_NAME -> listOf(CardRole.CARDHOLDER_FAMILY_NAME)
        CardKind.NUMBER -> listOf(CardRole.NUMBER)
        CardKind.VERIFICATION_NUMBER -> listOf(CardRole.VERIFICATION_NUMBER)
        CardKind.EXPIRY -> listOf(CardRole.EXPIRY_MONTH, CardRole.EXPIRY_YEAR)
        CardKind.EXPIRY_MONTH -> listOf(CardRole.EXPIRY_MONTH)
        CardKind.EXPIRY_YEAR -> listOf(CardRole.EXPIRY_YEAR)
        CardKind.BRAND -> listOf(CardRole.BRAND)
    }

    /** A number; or a code and an expiry; or fields all named by hints (a processor's frame). */
    private fun qualifies(frame: CardFrame): Boolean {
        val kinds = frame.fields.map { it.kind }.toSet()
        val codeAndExpiry = CardKind.VERIFICATION_NUMBER in kinds &&
            (CardKind.EXPIRY in kinds || CardKind.EXPIRY_MONTH in kinds)
        return CardKind.NUMBER in kinds || codeAndExpiry || frame.fields.all { it.byHint }
    }

    private fun classify(inFrame: List<FieldFacts>): List<CardField> {
        val bounded = inFrame.take(MAX_FRAME_FIELDS)
        val first = bounded.map { it to CardFieldClassifier.kindOf(it, strong = false) }
        val strong = first.any { (_, k) -> k?.kind in STRONG }
        val found = first.mapNotNull { (f, k) ->
            val of = k ?: if (strong) CardFieldClassifier.kindOf(f, strong = true) else null
            of?.let { CardField(f.index, it.kind, it.byHint) }
        }
        return splitNumber(found, bounded)
    }

    /** A number typed into 3–5 adjacent boxes of 4–6 characters: one slice each. */
    private fun splitNumber(found: List<CardField>, inFrame: List<FieldFacts>): List<CardField> {
        val run = numberRun(found, inFrame)
        if (run.isEmpty()) return found
        var start = 0
        val byHint = found.first { it.kind == CardKind.NUMBER }.byHint
        val slices = run.associate { f ->
            val length = maxLengthOf(f)
            val slice = CardField(f.index, CardKind.NUMBER, byHint, start until start + length)
            start += length
            f.index to slice
        }
        return inFrame.mapNotNull { f -> slices[f.index] ?: found.firstOrNull { it.index == f.index } }
    }

    private fun numberRun(found: List<CardField>, inFrame: List<FieldFacts>): List<FieldFacts> {
        val number = found.firstOrNull { it.kind == CardKind.NUMBER }
        val at = number?.let { n -> inFrame.indexOfFirst { it.index == n.index } } ?: -1
        val run = if (at < 0) {
            emptyList()
        } else {
            inFrame.drop(at).take(MAX_BOXES).takeWhile { f ->
                maxLengthOf(f) in BOX_LENGTHS && found.none { it.index == f.index && it.kind != CardKind.NUMBER }
            }
        }
        return run.takeIf { it.size >= MIN_BOXES && it.sumOf(::maxLengthOf) in NUMBER_LENGTHS }.orEmpty()
    }

    private val STRONG = setOf(CardKind.NUMBER, CardKind.VERIFICATION_NUMBER, CardKind.EXPIRY)
    private const val MAX_FRAME_FIELDS = 60
    private const val MIN_BOXES = 3
    private const val MAX_BOXES = 5
    private const val MIN_BOX_LENGTH = 4
    private const val MAX_BOX_LENGTH = 6
    private const val MIN_NUMBER_LENGTH = 13
    private const val MAX_NUMBER_LENGTH = 19
    private val BOX_LENGTHS = MIN_BOX_LENGTH..MAX_BOX_LENGTH
    private val NUMBER_LENGTHS = MIN_NUMBER_LENGTH..MAX_NUMBER_LENGTH
}
