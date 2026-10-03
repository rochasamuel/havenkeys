package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotSelected
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ShellBarsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun theBottomBarHasThreeTabsAndMarksTheSelectedOne() {
        val picked = mutableListOf<Tab>()
        rule.setKit { BottomBar(Tab.ITEMS, onSelect = { picked += it }) }
        rule.onNode(hasText("Items") and hasRole(Role.Tab)).assertIsSelected().assertTouchTarget()
        rule.onNode(hasText("Home") and hasRole(Role.Tab)).assertIsNotSelected().assertTouchTarget()
        rule.onNode(hasText("Settings") and hasRole(Role.Tab)).assertIsNotSelected().performClick()
        // A reselect is reported too: the shell pops that tab to its root.
        rule.onNode(hasText("Items") and hasRole(Role.Tab)).performClick()
        assertEquals(listOf(Tab.SETTINGS, Tab.ITEMS), picked)
    }

    private val done = mutableListOf<String>()
    private val actions = TopBarActions(
        onSearch = { done += "search" },
        onSync = { done += "sync" },
        onLock = { done += "lock" },
    )

    @Test
    fun theTopBarSearchesSyncsAndLocks() {
        rule.setKit { ShellTopBar(online = true, syncing = false, actions = actions) }
        rule.onNode(hasText("Search HavenKeys") and hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.onNodeWithContentDescription("Sync now").assertTouchTarget().performClick()
        rule.onNodeWithContentDescription("Lock now").assertTouchTarget().performClick()
        assertEquals(listOf("search", "sync", "lock"), done)
        rule.onNodeWithText("Offline").assertDoesNotExist()
    }

    @Test
    fun whileSyncingTheButtonSaysSoAndDoesNotSyncTwice() {
        rule.setKit { ShellTopBar(online = true, syncing = true, actions = actions) }
        rule.onNodeWithContentDescription("Sync now").assertDoesNotExist()
        rule.onNodeWithContentDescription("Syncing").assertIsDisplayed()
        assertEquals(emptyList<String>(), done)
    }

    @Test
    fun offlineTheTopBarSaysSo() {
        rule.setKit { ShellTopBar(online = false, syncing = false, actions = actions) }
        rule.onNodeWithText("Offline").assertIsDisplayed()
    }
}
