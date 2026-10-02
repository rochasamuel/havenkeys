package net.havenkeys.android.autofill

import android.text.InputType

// Builders for recorded structures. Indices only grow, so fields built in
// order keep their screen order.
private var next = 0

fun field(
    hints: List<String> = emptyList(),
    inputType: Int = InputType.TYPE_CLASS_TEXT,
    id: String? = null,
    hint: String? = null,
    html: Map<String, String> = emptyMap(),
    visible: Boolean = true,
    enabled: Boolean = true,
    focused: Boolean = false,
    domain: String? = null,
    scheme: String? = null,
    maxTextLength: Int = -1,
) = FieldFacts(
    next++, hints, inputType, id, hint, null, if (html.isEmpty()) null else "input", html,
    visible, enabled, focused, domain, scheme, maxTextLength,
)

val password = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
val webPassword = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
val email = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
