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
     * Android documents that the history is reset when the service answers
     * (onSuccess/onFailure). This ledger guards the paths where it is read
     * again without a new answer (a cancelled request) and repeated reads of
     * a growing history: it remembers the SELECTED events already counted and
     * counts only the new tail. Other events never matter.
     */
    class Ledger {
        private var counted: List<String> = emptyList()

        /** The items to count for [events], read now. */
        fun fresh(events: List<Picked>): List<String> {
            val picked = usedItems(events)
            val isGrowth = picked.size >= counted.size && picked.subList(0, counted.size) == counted
            val news = if (isGrowth) picked.drop(counted.size) else picked
            counted = picked
            return news
        }

        /** A non-null response was returned: a new history starts. */
        fun responded() {
            counted = emptyList()
        }
    }

    fun usedItems(events: List<Picked>): List<String> =
        events.filter { it.kind == Kind.SELECTED }.mapNotNull { itemOf(it.datasetId) }
}
