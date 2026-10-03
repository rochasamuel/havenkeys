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
                item("1", ItemKind.LOGIN, "bank"),
                item("2", ItemKind.LOGIN, "Amazon", passkey = true),
                item("3", ItemKind.SECURE_NOTE, "Wi-Fi"),
                item("4", ItemKind.IDENTITY, "Sam"),
            ),
        )
    }

    private fun vm() = ItemListViewModel(vault, FakeAccountRepository(), VaultEventsHub())

    @Test
    fun theItemsTabCountsEachCategoryAndOpensIt() {
        val vm = vm()
        val opened = mutableListOf<Category>()
        rule.setKit { ItemsScreen(vm, onCategory = { opened += it }, contentPadding = PaddingValues()) }
        rule.onNodeWithText("Items").assert(isHeading())
        rule.onNode(hasText("All items") and hasClickAction()).assert(hasText("4"))
        rule.onNode(hasText("Logins") and hasClickAction()).assert(hasText("2"))
        rule.onNode(hasText("Cards") and hasClickAction()).assert(hasText("0"))
        rule.onNode(hasText("Passkeys") and hasClickAction()).assert(hasText("1")).performClick()
        assertEquals(listOf(Category.PASSKEYS), opened)
    }

    @Test
    fun aCategoryListsItsItemsAToZAndOpensThem() {
        val vm = vm()
        val opened = mutableListOf<Pair<String, String>>()
        var back = 0
        rule.setKit {
            CategoryScreen(
                vm,
                Category.LOGINS,
                onOpen = { id, origin -> opened += id to origin },
                onBack = { back++ },
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
        rule.onNodeWithContentDescription("Back").performClick()
        assertEquals(1, back)
    }

    @Test
    fun anEmptyCategorySaysSo() {
        val vm = vm()
        rule.setKit {
            CategoryScreen(vm, Category.CARDS, onOpen = { _, _ -> }, onBack = {}, contentPadding = PaddingValues())
        }
        rule.onNodeWithText("Nothing here yet").assertIsDisplayed()
    }

    @Test
    fun theBackChevronsGlyphLinesUpWithTheLargeTitle() {
        val vm = vm()
        rule.setKit {
            CategoryScreen(vm, Category.LOGINS, onOpen = { _, _ -> }, onBack = {}, contentPadding = PaddingValues())
        }
        val title = rule.onNodeWithText("Logins").getUnclippedBoundsInRoot()
        val back = rule.onNodeWithContentDescription("Back").getUnclippedBoundsInRoot()
        // A 22dp glyph centred in the 48dp target: its ink starts 13dp in, on the title's start.
        assertEquals(title.left.value, back.left.value + 13f, 0.5f)
    }

    private fun item(id: String, kind: ItemKind, title: String, passkey: Boolean = false) =
        ItemSummary(id, kind, title, null, null, false, passkey, 0, 0)
}
