package net.havenkeys.android.autofill

import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue

/**
 * What a card or identity row fills, by [FieldFacts.index]: only fields
 * that showed no value, each in its shape. Nothing is ever overwritten.
 */
object WalletEntries {
    fun card(
        fields: List<FieldFacts>,
        frames: List<CardFrame>,
        values: List<List<CardValue>>,
    ): List<Pair<Int, Shaped>> =
        frames.zip(values).flatMap { (frame, frameValues) ->
            val byRole = frameValues.associate { it.role to it.value }
            frame.fields.mapNotNull { field ->
                empty(fields, field.index)?.let { f -> ValueShaper.card(f, field, byRole)?.let { field.index to it } }
            }
        }

    fun identity(fields: List<FieldFacts>, form: IdentityForm, values: List<IdentityValue>): List<Pair<Int, Shaped>> {
        val byRole = values.associate { it.role to it.value }
        return form.fields.mapNotNull { field ->
            val value = byRole[field.role]
            val f = empty(fields, field.index)
            if (value == null || f == null) {
                null
            } else {
                ValueShaper.identity(f, field.role, value)?.let { field.index to it }
            }
        }
    }

    /** The empty card fields a gated or unlock row names, so Android shows the row there. */
    fun cardTargets(fields: List<FieldFacts>, frames: List<CardFrame>): List<Int> =
        frames.flatMap { it.fields }.map { it.index }.distinct().filter { empty(fields, it) != null }

    fun identityTargets(fields: List<FieldFacts>, form: IdentityForm, roles: Collection<IdentityRole>): List<Int> =
        form.fields.filter { it.role in roles }.map { it.index }.filter { empty(fields, it) != null }

    private fun empty(fields: List<FieldFacts>, index: Int): FieldFacts? =
        fields.getOrNull(index)?.takeIf { it.index == index && it.isEmpty }
}
