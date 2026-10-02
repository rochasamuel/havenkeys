package net.havenkeys.android.autofill

import android.view.View

/**
 * What Android told us about one fillable field. Untrusted input (spec §7.1):
 * only a classification signal; Rust checks the target on its own.
 * Deliberately has no `text`: the field's current value is never kept.
 */
data class FieldFacts(
    val index: Int,
    val autofillHints: List<String>,
    val inputType: Int,
    val idEntry: String?,
    val hint: String?,
    val contentDescription: String?,
    val htmlTag: String?,
    val htmlAttributes: Map<String, String>,
    val visible: Boolean,
    val enabled: Boolean,
    val focused: Boolean,
    val webDomain: String?,
    val webScheme: String?,
    /** A native view's length limit (`ViewNode.maxTextLength`); -1 when unknown. */
    val maxTextLength: Int = -1,
    /** `View.AUTOFILL_TYPE_TEXT`, `_LIST` or `_DATE`. */
    val autofillType: Int = View.AUTOFILL_TYPE_TEXT,
    /**
     * Shows no value (a list: nothing or its first option chosen). Read
     * only so a card or identity value never replaces what is there; the
     * value itself is not kept.
     */
    val isEmpty: Boolean = true,
    /** A list's option labels, in order, bounded. */
    val options: List<String> = emptyList(),
) {
    /** Its identifiers and labels normalized once, shared by every classifier in a request. */
    internal val words: FieldWords by lazy(LazyThreadSafetyMode.PUBLICATION) { FieldWords(this) }
}

val FieldFacts.isList: Boolean get() = autofillType == View.AUTOFILL_TYPE_LIST
val FieldFacts.isDate: Boolean get() = autofillType == View.AUTOFILL_TYPE_DATE
