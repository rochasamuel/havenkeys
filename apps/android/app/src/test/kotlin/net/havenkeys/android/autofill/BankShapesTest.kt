package net.havenkeys.android.autofill

import android.text.InputType
import net.havenkeys.android.autofill.BankShapes.BoxFacts
import org.junit.Assert.assertEquals
import org.junit.Test

/** The extension's bank-fill.test.ts shapeBank table, row for row. */
class BankShapesTest {
    private fun text(maxLength: Int?, hasDigitBox: Boolean = false) = BoxFacts("text", maxLength, hasDigitBox)

    @Test
    fun shapesEachValueToItsBox() {
        val table = listOf(
            Triple("12345" to "6", text(7), "12345-6"),
            Triple("12345" to "6", text(6), "123456"),
            Triple("12345" to "6", text(5), "12345"),
            Triple("12345" to "6", BoxFacts("number", null, false), "123456"),
            Triple("12345" to "6", text(null, hasDigitBox = true), "12345"),
            Triple("1234" to "X", text(6), "1234-X"),
            Triple("12345" to null, text(7), "12345"),
            Triple("12345" to "6", text(null), "12345-6"),
        )
        for ((input, box, expected) in table) {
            assertEquals("$input in $box", expected, BankShapes.shape(input.first, input.second, box))
        }
    }

    @Test
    fun aBoxsFactsComeFromItsField() {
        val number = field(html = mapOf("type" to "number", "maxlength" to "7"))
        assertEquals(BoxFacts("number", 7, false), BankShapes.boxOf(number, false))
        assertEquals(BoxFacts("tel", null, true), BankShapes.boxOf(field(html = mapOf("type" to "tel")), true))
        assertEquals(BoxFacts("text", null, false), BankShapes.boxOf(field(hint = "Conta"), false))
        // A native number box takes no hyphen.
        assertEquals("number", BankShapes.boxOf(field(inputType = InputType.TYPE_CLASS_NUMBER), false).type)
    }
}
