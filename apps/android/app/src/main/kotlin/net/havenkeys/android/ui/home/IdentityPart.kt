package net.havenkeys.android.ui.home

import androidx.annotation.StringRes
import net.havenkeys.android.R

/**
 * The kinds of detail an identity holds, for Home's identity card. Built
 * from the names of the filled fields that `item_view` lists without their
 * values (havenkeys-mobile items.rs), so Home never reveals anything.
 */
enum class IdentityPart(@StringRes val label: Int, internal val fields: Set<String>) {
    NAME(R.string.identity_part_name, setOf("first_name", "middle_name", "last_name")),
    EMAIL(R.string.identity_part_email, setOf("email")),
    PHONE(R.string.identity_part_phone, setOf("mobile_phone", "home_phone", "work_phone")),
    ADDRESS(
        R.string.identity_part_address,
        setOf("street", "number", "complement", "neighborhood", "city", "state", "postal_code", "country"),
    ),
    WORK(R.string.identity_part_work, setOf("occupation", "company", "job_title")),
    DOCUMENTS(R.string.identity_part_documents, setOf("cpf", "rg", "passport", "drivers_license")),

    /** Any identity field no other part lists (gender, birth date, username, website, notes). */
    OTHER(R.string.identity_part_other, emptySet()),
    ;

    companion object {
        private const val PREFIX = "identity."

        /** The parts that a view's field keys (`identity.<name>`) fill, in this order. */
        fun of(keys: List<String>): List<IdentityPart> {
            val names = keys.filter { it.startsWith(PREFIX) }.map { it.removePrefix(PREFIX) }.toSet()
            val listed = entries.flatMap { it.fields }.toSet()
            return entries.filter { part ->
                if (part == OTHER) names.any { it !in listed } else names.any { it in part.fields }
            }
        }
    }
}
