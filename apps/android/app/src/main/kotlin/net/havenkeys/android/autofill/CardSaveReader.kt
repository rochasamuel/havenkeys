package net.havenkeys.android.autofill

import android.app.assist.AssistStructure
import android.content.pm.PackageManager
import uniffi.havenkeys_mobile.FrameFacts
import uniffi.havenkeys_mobile.SaveCard
import uniffi.havenkeys_mobile.TargetFacts

/** A card the user typed and confirmed in Android's save sheet. Holds the number: never logged, never kept. */
class SubmittedCard(val target: TargetFacts, val frame: FrameFacts, val card: SaveCard) {
    override fun toString() = "SubmittedCard(…)"
}

/** A card as typed into one frame's fields. */
object TypedCard {
    private const val MIN_DIGITS = 12

    fun collect(frame: CardFrame, valueAt: (Int) -> String?): SaveCard? {
        fun first(kind: CardKind) = frame.fields.firstOrNull { it.kind == kind }
            ?.let { valueAt(it.index) }?.trim()?.takeIf { it.isNotEmpty() }
        val number = frame.fields.filter { it.kind == CardKind.NUMBER }
            .joinToString("") { valueAt(it.index).orEmpty() }
        val name = first(CardKind.CARDHOLDER_NAME)
            ?: listOfNotNull(first(CardKind.CARDHOLDER_GIVEN_NAME), first(CardKind.CARDHOLDER_FAMILY_NAME))
                .joinToString(" ").ifEmpty { null }
        val expiry = ExpiryText.of(first(CardKind.EXPIRY), first(CardKind.EXPIRY_MONTH), first(CardKind.EXPIRY_YEAR))
        return if (number.count(Char::isDigit) < MIN_DIGITS) {
            null
        } else {
            SaveCard(name, number, first(CardKind.VERIFICATION_NUMBER), expiry)
        }
    }
}

/**
 * Reads a confirmed card save: the newest fill context with a typed
 * number, and only its card fields' values.
 */
class CardSaveReader(private val pm: PackageManager) {
    fun read(structures: List<AssistStructure>): SubmittedCard? =
        structures.asReversed().firstNotNullOfOrNull(::readOne)

    private fun readOne(structure: AssistStructure): SubmittedCard? {
        val parser = StructureParser()
        val screen = parser.parse(structure)
        val frame = CardFormFinder.find(screen.fields, useFocus = false)?.frames?.first()
        val ids = frame?.fields.orEmpty()
            .mapNotNull { f -> screen.ids.getOrNull(f.index)?.let { f.index to it } }.toMap()
        val text = parser.textOf(structure, ids.values.toSet())
        val card = frame?.let { TypedCard.collect(it) { index -> ids[index]?.let(text::get) } }
        return if (frame == null || card == null || screen.packageName.isEmpty()) {
            null
        } else {
            SubmittedCard(
                pm.targetOf(screen, screen.pageDomain, screen.pageScheme),
                FrameFacts(frame.webDomain, frame.webScheme),
                card,
            )
        }
    }
}
