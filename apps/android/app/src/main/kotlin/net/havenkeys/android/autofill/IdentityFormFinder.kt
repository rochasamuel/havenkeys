package net.havenkeys.android.autofill

import uniffi.havenkeys_mobile.IdentityRole

data class IdentityField(val index: Int, val role: IdentityRole)

/** The identity fields of one frame, in screen order. */
data class IdentityForm(val fields: List<IdentityField>, val webDomain: String?, val webScheme: String?) {
    val roles: List<IdentityRole> get() = fields.map { it.role }.distinct()
}

/**
 * The identity group around the focused field (identity.ts): its frame's
 * identity fields, when there are two, or one named by a hint that is not
 * an email or username box (those are a login's first step).
 */
object IdentityFormFinder {
    fun find(fields: List<FieldFacts>): IdentityForm? {
        val roles = fields.associate {
            it.index to if (it.visible && it.enabled) IdentityFieldClassifier.roleOf(it) else null
        }
        val anchor = fields.firstOrNull { it.focused } ?: fields.firstOrNull { roles[it.index] != null }
        val found = anchor?.let { a ->
            fields.filter { it.webDomain == a.webDomain && it.webScheme == a.webScheme }
                .mapNotNull { f -> roles[f.index]?.let { f.index to it } }
                .take(MAX_FIELDS)
        }.orEmpty()
        val qualifies = found.size >= 2 ||
            found.singleOrNull()?.second?.let { it.byHint && it.role !in LOGIN_ROLES } == true
        return if (anchor != null && qualifies && found.any { it.first == anchor.index }) {
            IdentityForm(found.map { (i, r) -> IdentityField(i, r.role) }, anchor.webDomain, anchor.webScheme)
        } else {
            null
        }
    }

    private val LOGIN_ROLES = setOf(IdentityRole.EMAIL, IdentityRole.USERNAME)
    private const val MAX_FIELDS = 60
}
