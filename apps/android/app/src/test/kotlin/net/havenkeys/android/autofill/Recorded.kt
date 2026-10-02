package net.havenkeys.android.autofill

import android.text.InputType
import android.view.View

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
    tag: String? = null,
    autofillType: Int = View.AUTOFILL_TYPE_TEXT,
    isEmpty: Boolean = true,
    options: List<String> = emptyList(),
    contentDescription: String? = null,
) = FieldFacts(
    next++, hints, inputType, id, hint, contentDescription, tag ?: if (html.isEmpty()) null else "input", html,
    visible, enabled, focused, domain, scheme, maxTextLength, autofillType, isEmpty, options,
)

/** A list (an HTML select or a spinner). */
fun list(
    options: List<String>,
    id: String? = null,
    hints: List<String> = emptyList(),
    html: Map<String, String> = emptyMap(),
    focused: Boolean = false,
    domain: String? = null,
    scheme: String? = null,
    isEmpty: Boolean = true,
) = field(
    hints = hints, id = id, html = html, focused = focused, domain = domain, scheme = scheme,
    tag = if (html.isEmpty()) null else "select", autofillType = View.AUTOFILL_TYPE_LIST,
    isEmpty = isEmpty, options = options,
)

/** A date picker field. */
fun date(hints: List<String> = emptyList(), id: String? = null, focused: Boolean = false) =
    field(hints = hints, id = id, focused = focused, autofillType = View.AUTOFILL_TYPE_DATE)

val password = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_PASSWORD
val webPassword = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_WEB_PASSWORD
val email = InputType.TYPE_CLASS_TEXT or InputType.TYPE_TEXT_VARIATION_EMAIL_ADDRESS
