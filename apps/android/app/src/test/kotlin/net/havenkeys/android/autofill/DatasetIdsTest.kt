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
