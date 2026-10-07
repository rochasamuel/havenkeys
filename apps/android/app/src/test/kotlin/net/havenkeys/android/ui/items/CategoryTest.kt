package net.havenkeys.android.ui.items

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

class CategoryTest {
    private fun item(kind: ItemKind, passkey: Boolean = false) =
        ItemSummary("id", kind, "Title", null, null, false, passkey, 0, 0, tags = emptyList())

    private val all = listOf(
        item(ItemKind.LOGIN),
        item(ItemKind.LOGIN, passkey = true),
        item(ItemKind.SECURE_NOTE),
        item(ItemKind.CARD),
        item(ItemKind.IDENTITY),
    )

    @Test
    fun eachCategoryKeepsItsKind() {
        assertEquals(5, all.count(Category.ALL::keeps))
        assertEquals(2, all.count(Category.LOGINS::keeps))
        assertEquals(1, all.count(Category.PASSKEYS::keeps))
        assertEquals(1, all.count(Category.NOTES::keeps))
        assertEquals(1, all.count(Category.CARDS::keeps))
    }

    @Test
    fun onlyTheFiveCategoriesParse() {
        Category.entries.forEach { assertEquals(it, Category.fromArg(it.arg)) }
        assertNull("the identity is on Home, not a list", Category.fromArg("identity"))
        assertNull(Category.fromArg("../vault"))
        assertNull(Category.fromArg(""))
        assertNull(Category.fromArg(null))
    }
}
