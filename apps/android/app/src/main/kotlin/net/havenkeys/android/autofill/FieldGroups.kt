package net.havenkeys.android.autofill

import android.view.View

/**
 * The group a field belongs with: [id] names one node of the structure.
 * [form]: that node is an HTML `<form>` (a browser's or WebView's page);
 * otherwise it is a container found by climbing (a native view group, or a
 * page's element when the field has no form).
 */
data class FieldGroup(val id: Int, val form: Boolean)

/** One accepted field as the parser saw it, for [FieldGroups.assign]. */
internal class GroupInput(
    /** The nearest HTML `<form>` ancestor's node id. */
    val form: Int?,
    /** Ancestor node ids, nearest first (at least [FieldGroups.MAX_CLIMB] of them when the tree has them). */
    val ancestors: List<Int>,
    /** A text field (not a list or a date): what makes a container a group. */
    val text: Boolean,
)

/**
 * Which fields belong together, and the focused field's group (spec
 * 2026-11-14-android-email-username-focused-field §4). The grouping is the
 * extension's `groupRoot` (apps/extension/src/autofill/group.ts): the
 * field's form when it has one of at most [MAX_FORM_INPUTS] fields,
 * otherwise the nearest ancestor, at most [MAX_CLIMB] levels up, that also
 * holds another text field.
 */
internal object FieldGroups {
    const val MAX_CLIMB = 8

    /** A form with more fields than this wraps a whole page (ASP.NET style): not a group. */
    const val MAX_FORM_INPUTS = 40

    /** Each input's group, in order. Linear: each field adds itself to at most [MAX_CLIMB] ancestors. */
    fun assign(inputs: List<GroupInput>): List<FieldGroup?> {
        val formSize = HashMap<Int, Int>()
        val total = HashMap<Int, Int>()
        val texts = HashMap<Int, Int>()
        for (input in inputs) {
            input.form?.let { formSize.merge(it, 1, Int::plus) }
            for (a in input.ancestors.take(MAX_CLIMB)) {
                total.merge(a, 1, Int::plus)
                if (input.text) texts.merge(a, 1, Int::plus)
            }
        }
        return inputs.map { input ->
            val form = input.form
            if (form != null && formSize.getValue(form) <= MAX_FORM_INPUTS) {
                FieldGroup(form, form = true)
            } else {
                climb(input, total, texts)
            }
        }
    }

    private fun climb(input: GroupInput, total: Map<Int, Int>, texts: Map<Int, Int>): FieldGroup? {
        // Up through containers of at most a form's worth of fields, to the
        // first one holding another text field: a username beside its
        // password, a password beside its username.
        val within = input.ancestors.take(MAX_CLIMB).takeWhile { total.getValue(it) <= MAX_FORM_INPUTS }
        val node = within.firstOrNull { (texts[it] ?: 0) >= 2 } ?: within.lastOrNull()
        return node?.let { FieldGroup(it, form = false) }
    }

    /**
     * The fields that belong with the focused field: its form, or its view
     * group. Every field when nothing is focused or the focused field has no
     * group (the behaviour before groups).
     *
     * A view group (not an HTML form) is used only when it holds a password
     * or code field of its own, or the focused field's frame has none
     * outside it: Android layouts wrap each box and put boxes in rows, so a
     * view group can split a username from its password, and a fill must
     * never lose the password it had.
     */
    fun focusedGroup(fields: List<FieldFacts>): List<FieldFacts> {
        val focused = fields.firstOrNull { it.focused }
        val group = focused?.group ?: return fields
        val members = fields.filter { it.group == group }
        return if (group.form || keepsItsSignIn(fields, focused, group)) members else fields
    }

    /** [focused]'s view [group] holds a password or code of its own, or its frame has none. */
    private fun keepsItsSignIn(fields: List<FieldFacts>, focused: FieldFacts, group: FieldGroup): Boolean {
        val roles = loginRolesOf(fields)
        val signIn = fields.indices.filter { roles[it].role in SIGN_IN_ROLES && sameFrame(fields[it], focused) }
        return signIn.isEmpty() || signIn.any { fields[it].group == group }
    }

    /** In the same frame: the same web domain and scheme (both null in an app). */
    fun sameFrame(a: FieldFacts, b: FieldFacts) = a.webDomain == b.webDomain && a.webScheme == b.webScheme

    /** A text field: what makes a container a group. */
    fun isText(autofillType: Int) = autofillType == View.AUTOFILL_TYPE_TEXT

    private val SIGN_IN_ROLES =
        setOf(FieldRole.PASSWORD, FieldRole.NEW_PASSWORD, FieldRole.CONFIRM_PASSWORD, FieldRole.OTP)
}
