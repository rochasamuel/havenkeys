package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class CardSaveTest {
    private val frame = CardFrame(
        null, null,
        listOf(
            CardField(0, CardKind.CARDHOLDER_NAME, false),
            CardField(1, CardKind.NUMBER, false, 0 until 4),
            CardField(2, CardKind.NUMBER, false, 4 until 8),
            CardField(3, CardKind.NUMBER, false, 8 until 12),
            CardField(4, CardKind.NUMBER, false, 12 until 16),
            CardField(5, CardKind.EXPIRY, false),
            CardField(6, CardKind.VERIFICATION_NUMBER, false),
        ),
    )

    @Test
    fun aSplitNumberIsJoinedAndTheExpiryRead() {
        val typed = listOf("Ana Souza", "4000", "0566", "5566", "5556", "07/31", "321")
        val card = TypedCard.collect(frame) { typed.getOrNull(it) }!!
        assertEquals("4000056655665556", card.number)
        assertEquals("2031-07", card.expiry)
        assertEquals("321", card.verificationNumber)
        assertEquals("Ana Souza", card.cardholderName)
    }

    @Test
    fun noNumberNoSave() {
        val typed = listOf("Ana Souza", "", "", "", "", "07/31", "321")
        assertNull(TypedCard.collect(frame) { typed.getOrNull(it) })
    }

    @Test
    fun aCardNeverPrintsItself() {
        val card = TypedCard.collect(frame) { listOf("A", "4000", "0566", "5566", "5556").getOrNull(it) }!!
        val submitted = SubmittedCard(
            uniffi.havenkeys_mobile.TargetFacts("p", emptyList(), null, null),
            uniffi.havenkeys_mobile.FrameFacts(null, null),
            card,
        )
        assertEquals("SubmittedCard(…)", submitted.toString())
    }
}
