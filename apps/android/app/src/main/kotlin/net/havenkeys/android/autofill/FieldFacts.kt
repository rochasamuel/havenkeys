package net.havenkeys.android.autofill

/**
 * What Android told us about one fillable field. Untrusted input (spec §7.1):
 * only a classification signal; Rust checks the target on its own.
 * Deliberately has no `text`: the field's current value is never read.
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
)
