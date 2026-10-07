package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class ItemsScreensTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(
            listOf(
                item("1", ItemKind.LOGIN, "bank", tags = listOf("staging", "work")),
                item("2", ItemKind.LOGIN, "Amazon", passkey = true, tags = listOf("staging")),
                item("3", ItemKind.SECURE_NOTE, "Wi-Fi"),
                item("4", ItemKind.IDENTITY, "Sam"),
            ),
        )
    }

    private val events = VaultEventsHub()

    private fun vm() = ItemListViewModel(vault, FakeAccountRepository(), events)

    @Test
    fun theItemsTabCountsEachCategoryAndOpensIt() {
        val vm = vm()
        val opened = mutableListOf<Category>()
        rule.setKit { ItemsScreen(vm, onCategory = { opened += it }, onTag = {}, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Items").assert(isHeading())
        rule.onNode(hasText("All items") and hasClickAction()).assert(hasText("4"))
        rule.onNode(hasText("Logins") and hasClickAction()).assert(hasText("2"))
        rule.onNode(hasText("Cards") and hasClickAction()).assert(hasText("0"))
        rule.onNode(hasText("Passkeys") and hasClickAction()).assert(hasText("1")).performClick()
        assertEquals(listOf(Category.PASSKEYS), opened)
    }

    @Test
    fun theItemsTabListsEachTagWithItsCountAndOpensIt() {
        val vm = vm()
        val opened = mutableListOf<String>()
        rule.setKit { ItemsScreen(vm, onCategory = {}, onTag = { opened += it }, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Tags").assert(isHeading())
        rule.onNode(hasText("work") and hasClickAction()).assert(hasText("1"))
        rule.onNode(hasText("staging") and hasClickAction()).assert(hasText("2")).performClick()
        assertEquals(listOf("staging"), opened)
    }

    @Test
    fun withNoTagsTheItemsTabHasNoTagsGroup() {
        vault.items = Outcome.Ok(listOf(item("3", ItemKind.SECURE_NOTE, "Wi-Fi")))
        val vm = vm()
        rule.setKit { ItemsScreen(vm, onCategory = {}, onTag = {}, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Tags").assertDoesNotExist()
    }

    @Test
    fun aTagListsOnlyItsItems() {
        val vm = vm()
        rule.setKit {
            TagScreen(vm, "work", onOpen = { _, _ -> }, onGone = {}, contentPadding = PaddingValues())
        }
        rule.onNodeWithText("work").assert(isHeading())
        rule.onNode(hasText("bank") and hasClickAction()).assertIsDisplayed()
        rule.onNodeWithText("Amazon").assertDoesNotExist()
    }

    /** Final review (tags): the list of a tag no item carries any more stayed open, empty. */
    @Test
    fun aTagNoItemCarriesAnyMoreClosesItsList() {
        val vm = vm()
        var gone = 0
        rule.setKit {
            TagScreen(vm, "work", onOpen = { _, _ -> }, onGone = { gone++ }, contentPadding = PaddingValues())
        }
        rule.runOnIdle { assertEquals(0, gone) }
        // Its last item untagged here, or by a sync: the vault's list no longer has it.
        vault.items = Outcome.Ok(listOf(item("1", ItemKind.LOGIN, "bank", tags = listOf("staging"))))
        events.itemsChanged()
        rule.runOnIdle { assertEquals(1, gone) }
    }

    @Test
    fun aTagListStaysOpenThroughTheLockWipe() {
        val vm = vm()
        var gone = 0
        rule.setKit {
            TagScreen(vm, "work", onOpen = { _, _ -> }, onGone = { gone++ }, contentPadding = PaddingValues())
        }
        events.locked("user")
        rule.runOnIdle { assertEquals(0, gone) }
    }

    @Test
    fun aCategoryListsItsItemsAToZAndOpensThem() {
        val vm = vm()
        val opened = mutableListOf<Pair<String, String>>()
        rule.setKit {
            CategoryScreen(
                vm,
                Category.LOGINS,
                onOpen = { id, origin -> opened += id to origin },
                contentPadding = PaddingValues(),
            )
        }
        rule.onNodeWithText("Logins").assert(isHeading())
        val amazon = rule.onNode(hasText("Amazon") and hasClickAction()).getUnclippedBoundsInRoot().top
        val bank = rule.onNode(hasText("bank") and hasClickAction()).getUnclippedBoundsInRoot().top
        assertTrue(amazon < bank)
        rule.onNodeWithText("Wi-Fi").assertDoesNotExist()
        rule.onNode(hasText("bank") and hasClickAction()).performClick()
        assertEquals(listOf("1" to Origins.CATEGORY), opened)
        // The system Back returns to the categories: the list draws no chevron of its own.
        rule.onNodeWithContentDescription("Back").assertDoesNotExist()
    }

    @Test
    fun anEmptyCategorySaysSo() {
        val vm = vm()
        rule.setKit {
            CategoryScreen(vm, Category.CARDS, onOpen = { _, _ -> }, contentPadding = PaddingValues())
        }
        rule.onNodeWithText("Nothing here yet").assertIsDisplayed()
    }

    private fun item(
        id: String,
        kind: ItemKind,
        title: String,
        passkey: Boolean = false,
        tags: List<String> = emptyList(),
    ) = ItemSummary(id, kind, title, null, null, false, passkey, 0, 0, tags = tags)
}
