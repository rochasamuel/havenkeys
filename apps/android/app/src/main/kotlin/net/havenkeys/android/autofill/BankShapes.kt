package net.havenkeys.android.autofill

import android.text.InputType
import uniffi.havenkeys_mobile.BankFillRecord

/**
 * Rust's bank numbers, each in its box's shape (the extension's bank-fill.ts,
 * spec 2026-11-03-bank-account §5.2). Nothing is cut to fit: a value longer
 * than its box is left out.
 */
object BankShapes {
    /** [maxLength] null when the box has none; [hasDigitBox]: the form has a box for this number's check digit. */
    data class BoxFacts(val type: String, val maxLength: Int?, val hasDigitBox: Boolean)

    /** [value] and its check [digit] in one box: `NNNN-D`, `NNNND`, or the number alone. */
    fun shape(value: String, digit: String?, box: BoxFacts): String {
        if (digit == null || box.hasDigitBox) return value
        fun room(n: Int) = box.maxLength == null || box.maxLength >= n
        return when {
            box.type != "number" && room(value.length + 1 + digit.length) -> "$value-$digit"
            room(value.length + digit.length) -> "$value$digit"
            else -> value
        }
    }

    /** A box's facts. A native number box, like `type=number`, takes no hyphen. */
    fun boxOf(f: FieldFacts, hasDigitBox: Boolean): BoxFacts {
        val html = f.htmlAttributes["type"]?.lowercase()?.ifEmpty { null }
        val inputClass = f.inputType and InputType.TYPE_MASK_CLASS
        val nativeNumber = f.htmlTag == null && inputClass == InputType.TYPE_CLASS_NUMBER
        val type = html ?: if (nativeNumber) "number" else "text"
        return BoxFacts(type, maxLengthOf(f).takeIf { it >= 0 }, hasDigitBox)
    }

    /** What each of [boxes] takes, by field index. A box whose value is empty or does not fit gets nothing. */
    fun values(boxes: List<BankBox>, fields: Map<Int, FieldFacts>, bank: BankFillRecord): List<Pair<Int, String>> {
        val roles = boxes.map { it.role }.toSet()
        return boxes.mapNotNull { box ->
            val f = fields[box.index] ?: return@mapNotNull null
            val value = when (box.role) {
                BankRole.BRANCH -> shape(bank.branch, bank.branchDigit, boxOf(f, BankRole.BRANCH_DIGIT in roles))
                BankRole.ACCOUNT -> shape(bank.account, bank.accountDigit, boxOf(f, BankRole.ACCOUNT_DIGIT in roles))
                BankRole.BRANCH_DIGIT -> bank.branchDigit
                BankRole.ACCOUNT_DIGIT -> bank.accountDigit
            }
            val max = maxLengthOf(f)
            value?.takeIf { it.isNotEmpty() && (max < 0 || it.length <= max) }?.let { box.index to it }
        }
    }
}
