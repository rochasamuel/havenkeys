package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.CardRole

class CardFormFinderTest {
    private fun kinds(frame: CardFrame) = frame.fields.map { it.kind }

    @Test
    fun aCheckoutWithNumberCodeAndExpiry() {
        val fields = listOf(
            field(hint = "Name on card"),
            field(hint = "Card number", focused = true),
            field(hint = "MM/YY", id = "expiry"),
            field(id = "cvc", maxTextLength = 4),
        )
        val form = CardFormFinder.find(fields)!!
        assertEquals(
            listOf(CardKind.CARDHOLDER_NAME, CardKind.NUMBER, CardKind.EXPIRY, CardKind.VERIFICATION_NUMBER),
            kinds(form.frames.single()),
        )
        assertEquals(
            listOf(
                CardRole.CARDHOLDER_NAME, CardRole.NUMBER, CardRole.EXPIRY_MONTH,
                CardRole.EXPIRY_YEAR, CardRole.VERIFICATION_NUMBER,
            ),
            CardFormFinder.rolesOf(form.frames.single()),
        )
    }

    @Test
    fun aNumberSplitOverFourBoxesGetsSlices() {
        val boxes = (0 until 4).map { field(id = "card_number_$it", maxTextLength = 4, focused = it == 0) }
        val fields = boxes + field(id = "cvv", maxTextLength = 3)
        val frame = CardFormFinder.find(fields)!!.frames.single()
        assertEquals(listOf(0 until 4, 4 until 8, 8 until 12, 12 until 16), frame.fields.take(4).map { it.slice })
        assertEquals(listOf(CardRole.NUMBER, CardRole.VERIFICATION_NUMBER), CardFormFinder.rolesOf(frame))
    }

    @Test
    fun processorFramesComeAfterTheFocusedOne() {
        val fields = listOf(
            field(hint = "Name on card", domain = "shop.example.com", scheme = "https"),
            field(
                html = mapOf("autocomplete" to "cc-number"), domain = "js.stripe.com", scheme = "https",
                focused = true,
            ),
            field(html = mapOf("autocomplete" to "cc-csc"), domain = "js.stripe.com", scheme = "https"),
        )
        val form = CardFormFinder.find(fields)!!
        assertEquals(listOf("js.stripe.com", "shop.example.com"), form.frames.map { it.webDomain })
    }

    @Test
    fun aFocusedFieldThatIsNotACardFieldFindsNothing() {
        val fields = listOf(field(hint = "Email", focused = true), field(hint = "Card number"))
        assertNull(CardFormFinder.find(fields))
        // A confirmed save reads the form whatever is focused.
        assertEquals(1, CardFormFinder.find(fields, useFocus = false)!!.frames.size)
    }

    @Test
    fun aLoneAmbiguousFieldDoesNotQualify() {
        assertNull(CardFormFinder.find(listOf(field(hint = "Name on card", focused = true))))
    }

    @Test
    fun hiddenFieldsAreLeftOutAndFramesAreBounded() {
        val hidden = field(hint = "Card number", visible = false, focused = true)
        assertNull(CardFormFinder.find(listOf(hidden)))
        val many = (0 until 12).map {
            field(
                html = mapOf("autocomplete" to "cc-number"), domain = "f$it.example", scheme = "https",
                focused = it == 0,
            )
        }
        assertEquals(CardFormFinder.MAX_CARD_FRAMES, CardFormFinder.find(many)!!.frames.size)
    }
}
