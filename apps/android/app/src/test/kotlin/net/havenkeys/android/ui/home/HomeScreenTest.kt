package net.havenkeys.android.ui.home

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.ViewField

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class HomeScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam@example.com", null, false, false, 0, 0)
    private val wifi = ItemSummary("2", ItemKind.SECURE_NOTE, "Wi-Fi", null, null, false, false, 0, 0)
    private val identity = ItemSummary("9", ItemKind.IDENTITY, "Sam", null, null, false, false, 0, 0)

    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github, wifi, identity))
        recent = Outcome.Ok(listOf(github, wifi))
        frequent = Outcome.Ok(listOf(github))
        view = Outcome.Ok(ItemView(identity, listOf(field("identity.first_name"), field("identity.email"))))
    }
    private val opened = mutableListOf<Pair<String, String>>()

    private fun show() {
        val vm = HomeViewModel(vault, FakeAccountRepository(), VaultEventsHub())
        rule.setKit {
            HomeScreen(vm, onOpen = { id, origin -> opened += id to origin }, contentPadding = PaddingValues())
        }
    }

    @Test
    fun theIdentitySitsOnTopAndSaysWhatItHolds() {
        show()
        rule.onNode(hasText("Sam") and hasClickAction()).assert(hasText("Name · Email")).performClick()
        assertEquals(listOf("9" to Origins.IDENTITY), opened)
    }

    @Test
    fun anEmptyIdentityAsksForDetails() {
        vault.view = Outcome.Ok(ItemView(identity, emptyList()))
        show()
        rule.onNode(hasText("Sam") and hasClickAction()).assert(hasText("Add your details"))
    }

    @Test
    fun bothListsShowUnderTheirTitles() {
        show()
        rule.onNodeWithText("Recently added").assert(isHeading())
        rule.onNodeWithText("Frequently used").assert(isHeading())
        rule.onNode(hasText("Wi-Fi") and hasClickAction()).assertIsDisplayed()
    }

    @Test
    fun anItemInBothListsOpensFromTheRowTapped() {
        show()
        val rows = rule.onAllNodes(hasText("GitHub") and hasClickAction())
        rows.assertCountEquals(2)
        rows[1].performClick()
        assertEquals(listOf("1" to Origins.FREQUENT), opened)
        rows[0].performClick()
        assertEquals("1" to Origins.RECENT, opened.last())
    }

    @Test
    fun noUsesYetSaysHowTheyCome() {
        vault.frequent = Outcome.Ok(emptyList())
        show()
        rule.onNodeWithText("Items you fill or copy will show here.").assertIsDisplayed()
    }

    private fun field(key: String) = ViewField(key, key, FieldKind.TEXT, null)

    @Test
    fun aFailedLoadSaysSoAndClaimsNoEmptyList() {
        vault.recent = Outcome.Failed("network")
        vault.frequent = Outcome.Ok(emptyList())
        show()
        rule.onNodeWithText("Something went wrong. Try again.").assertIsDisplayed()
        rule.onNodeWithText("New items you add appear here.").assertDoesNotExist()
        rule.onNodeWithText("Items you fill or copy will show here.").assertDoesNotExist()
    }
}
