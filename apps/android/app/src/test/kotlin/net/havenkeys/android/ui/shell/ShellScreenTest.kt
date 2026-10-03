package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository()
    private val accounts = FakeAccountRepository()
    private val events = VaultEventsHub()
    private val picks = mutableListOf<String>()

    private val screens = ShellScreens(
        home = { HavenText("Home root") },
        items = { _, onCategory -> HavenButton("Open logins", onClick = { onCategory(Category.LOGINS) }) },
        category = { _, category, _ -> HavenText("List ${category.arg}") },
        settings = { HavenText("Settings root") },
    )

    private fun show() {
        val vm = ShellViewModel(vault, accounts, events)
        val navigation = ShellNavigation(
            onSearch = { picks += "search" },
            onNew = { picks += "new:$it" },
            onGenerator = { picks += "generator" },
        )
        rule.setKit { ShellScreen(vm, screens, navigation) }
    }

    private fun tab(label: String) = rule.onNode(hasText(label) and hasRole(Role.Tab))

    @Test
    fun theBarsFrameTheCurrentTab() {
        show()
        rule.onNodeWithText("Home root").assertIsDisplayed()
        tab("Home").assertIsSelected()
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).assertIsDisplayed()
    }

    @Test
    fun theBottomBarSwitchesTabsAndAReselectPopsToTheRoot() {
        show()
        tab("Items").performClick()
        rule.onNodeWithText("Open logins").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        tab("Home").performClick()
        tab("Items").performClick()
        rule.onNodeWithText("List logins").assertIsDisplayed()
        tab("Items").performClick()
        rule.onNodeWithText("Open logins").assertIsDisplayed()
    }

    @Test
    fun settingsHasNoAddButton() {
        show()
        rule.onNodeWithContentDescription("New item").assertIsDisplayed()
        tab("Settings").performClick()
        rule.onNodeWithContentDescription("New item").assertDoesNotExist()
    }

    @Test
    fun offlineTheAddSheetOffersOnlyTheGenerator() {
        show()
        rule.onNodeWithContentDescription("New item").performClick()
        rule.onNode(hasText("Login") and hasRole(Role.Button)).assertIsNotEnabled()
        rule.onNode(hasText("Generate password") and hasRole(Role.Button)).performClick()
        assertEquals(listOf("generator"), picks)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun onlineAPickedTileOpensItsForm() {
        events.connectivity(true)
        show()
        rule.onNodeWithContentDescription("New item").performClick()
        rule.onNode(hasText("Secure note") and hasRole(Role.Button)).performClick()
        assertEquals(listOf("new:SECURE_NOTE"), picks)
    }

    @Test
    fun theTopBarSearchesSyncsAndLocks() {
        events.connectivity(true)
        show()
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).performClick()
        rule.onNodeWithContentDescription("Sync now").performClick()
        rule.onNodeWithContentDescription("Lock now").performClick()
        assertEquals(listOf("search"), picks)
        assertEquals(listOf("syncNow"), accounts.calls)
        assertTrue("lock" in vault.calls)
    }

    @Test
    fun aFailedSyncSaysWhyInAToast() {
        accounts.sync = Outcome.Failed("offline")
        events.connectivity(true)
        show()
        rule.onNodeWithContentDescription("Sync now").performClick()
        rule.onNodeWithText("HavenKeys is offline — the vault is read-only until it reconnects.").assertIsDisplayed()
    }
}
