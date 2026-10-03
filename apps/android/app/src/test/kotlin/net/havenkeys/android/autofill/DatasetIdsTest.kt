package net.havenkeys.android.autofill

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DatasetIdsTest {
    private val uuid = "7c9e6679-7425-40de-944b-e07fc1f90ae7"

    @Test
    fun anItemRowRoundTrips() = assertEquals(uuid, DatasetIds.itemOf(DatasetIds.of(uuid)))

    @Test
    fun otherRowsAndJunkAreNotItems() {
        assertNull(DatasetIds.itemOf(null))
        assertNull(DatasetIds.itemOf("search"))
        assertNull(DatasetIds.itemOf("item:"))
        assertNull(DatasetIds.itemOf("item:not-a-uuid"))
        assertNull(DatasetIds.itemOf("item:$uuid:extra"))
    }

    private fun pick(id: String = uuid) = listOf(DatasetIds.Picked(DatasetIds.Kind.SELECTED, DatasetIds.of(id)))

    @Test
    fun theSameHistoryWithoutAResponseCountsOnce() {
        val ledger = DatasetIds.Ledger()
        assertEquals(listOf(uuid), ledger.fresh(pick()))
        assertEquals(emptyList<String>(), ledger.fresh(pick()))
    }

    @Test
    fun theSameHistoryAfterAResponseCountsAgain() {
        val ledger = DatasetIds.Ledger()
        ledger.fresh(pick())
        ledger.responded()
        assertEquals(listOf(uuid), ledger.fresh(pick()))
    }

    @Test
    fun aDifferentHistoryCounts() {
        val ledger = DatasetIds.Ledger()
        ledger.fresh(pick())
        val other = "11111111-2222-4333-8444-555555555555"
        assertEquals(listOf(other), ledger.fresh(pick(other)))
    }

    @Test
    fun anOtherEventAfterACountedPickCountsNothing() {
        val ledger = DatasetIds.Ledger()
        ledger.fresh(pick())
        val grown = pick() + DatasetIds.Picked(DatasetIds.Kind.OTHER, null)
        assertEquals(emptyList<String>(), ledger.fresh(grown))
    }

    @Test
    fun aSecondPickOfTheSameRowCountsOnceMore() {
        val ledger = DatasetIds.Ledger()
        assertEquals(listOf(uuid), ledger.fresh(pick()))
        assertEquals(listOf(uuid), ledger.fresh(pick() + pick()))
        assertEquals(emptyList<String>(), ledger.fresh(pick() + pick()))
    }

    @Test
    fun onlyPickedDirectRowsCount() {
        val events = listOf(
            DatasetIds.Picked(DatasetIds.Kind.SELECTED, DatasetIds.of(uuid)),
            DatasetIds.Picked(DatasetIds.Kind.AUTHENTICATION_SELECTED, DatasetIds.of(uuid)),
            DatasetIds.Picked(DatasetIds.Kind.SELECTED, "search"),
            DatasetIds.Picked(DatasetIds.Kind.OTHER, DatasetIds.of(uuid)),
        )
        assertEquals(listOf(uuid), DatasetIds.usedItems(events))
    }
}
