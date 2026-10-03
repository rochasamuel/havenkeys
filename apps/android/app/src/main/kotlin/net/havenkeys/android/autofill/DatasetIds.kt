package net.havenkeys.android.autofill

import java.util.UUID

/**
 * Ids on direct-fill rows, so a picked row can count as a use of its item
 * (spec 2026-10-03-android-redesign §4.4). Only rows without authentication
 * carry one: a confirmed row counts in [AutofillAuthActivity] instead. An
 * item UUID is not a secret (routes carry it too).
 */
object DatasetIds {
    private const val PREFIX = "item:"

    fun of(itemId: String): String = PREFIX + itemId

    fun itemOf(datasetId: String?): String? {
        val rest = datasetId?.removePrefix(PREFIX)?.takeIf { it != datasetId }
        val parsed = rest?.let { runCatching { UUID.fromString(it) }.getOrNull() }
        return rest?.takeIf { parsed?.toString() == it.lowercase() }
    }

    enum class Kind { SELECTED, AUTHENTICATION_SELECTED, OTHER }

    /** One event of Android's fill event history, reduced to what counts. */
    data class Picked(val kind: Kind, val datasetId: String?)

    /**
     * Android keeps the history of the LAST response and does not drain it on
     * read, so a request that returns no new response would see the same pick
     * again. A ledger counts a history once until a response replaces it.
     */
    class Ledger {
        private var seen: List<Picked>? = null

        /** The items to count for [events], read now. */
        fun fresh(events: List<Picked>): List<String> {
            if (events == seen) return emptyList()
            seen = events
            return usedItems(events)
        }

        /** A non-null response was returned: a new history starts. */
        fun responded() {
            seen = null
        }
    }

    fun usedItems(events: List<Picked>): List<String> =
        events.filter { it.kind == Kind.SELECTED }.mapNotNull { itemOf(it.datasetId) }
}
