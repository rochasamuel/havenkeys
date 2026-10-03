package net.havenkeys.android.ui.search

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
class SearchScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        searches += listOf("bank", "mail")
    }
    private val opened = mutableListOf<Pair<String, String>>()
    private var cancelled = 0

    private fun show() {
        val vm = SearchViewModel(vault, events)
        rule.setKit {
            SearchScreen(vm, onOpen = { id, origin -> opened += id to origin }, onCancel = { cancelled++ })
        }
    }

    /** Results arrive after the typing pause, which runs on the real clock. */
    private fun awaitText(text: String) = rule.waitUntil(timeoutMillis = 5_000) {
        rule.onAllNodes(hasText(text)).fetchSemanticsNodes().isNotEmpty()
    }

    @Test
    fun anEmptyFieldShowsRecentSearchesWithClear() {
        show()
        rule.onNodeWithText("Recent searches").assert(isHeading())
        rule.onNode(hasText("bank") and hasClickAction()).assertIsDisplayed()
        rule.onNode(hasText("Clear") and hasRole(Role.Button)).performClick()
        rule.onNodeWithText("bank").assertDoesNotExist()
        assertEquals(emptyList<String>(), vault.searches)
    }

    @Test
    fun aRecentSearchFillsTheFieldAndRunsAgain() {
        show()
        rule.onNode(hasText("mail") and hasClickAction()).performClick()
        rule.onNode(hasSetTextAction()).assert(hasText("mail"))
        awaitText("GitHub")
        rule.onNode(hasText("GitHub") and hasClickAction()).assertIsDisplayed()
        assertEquals(listOf("bank", "mail"), vault.searches)
    }

    @Test
    fun typingShowsResultsAndOpeningOneRecordsTheQuery() {
        show()
        rule.onNode(hasSetTextAction()).performTextInput("git")
        awaitText("GitHub")
        assertEquals(listOf("bank", "mail"), vault.searches)
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        assertEquals(listOf("1" to Origins.SEARCH), opened)
        assertEquals(listOf("git", "bank", "mail"), vault.searches)
    }

    @Test
    fun fastTypingIsNotOverwrittenByTheViewModel() {
        show()
        val field = rule.onNode(hasSetTextAction())
        "github".forEach { field.performTextInput(it.toString()) }
        rule.waitForIdle()
        field.assert(hasText("github"))
    }

    @Test
    fun noMatchesSaysSo() {
        vault.items = Outcome.Ok(emptyList())
        show()
        rule.onNode(hasSetTextAction()).performTextInput("zzz")
        awaitText("No matches")
        rule.onNodeWithText("No matches").assertIsDisplayed()
    }

    @Test
    fun cancelLeaves() {
        show()
        rule.onNode(hasText("Cancel") and hasRole(Role.Button)).performClick()
        assertEquals(1, cancelled)
    }

    @Test
    fun aRestoredScreenDoesNotBringBackTheQuery() {
        val restoration = StateRestorationTester(rule)
        var vm = SearchViewModel(vault, events)
        restoration.setContent {
            HavenTheme { SearchScreen(vm, onOpen = { _, _ -> }, onCancel = {}) }
        }
        rule.onNode(hasSetTextAction()).performTextInput("private query")
        // The process came back: a new ViewModel, and only what was saved.
        vm = SearchViewModel(vault, events)
        restoration.emulateSavedInstanceStateRestore()
        rule.onNode(hasSetTextAction()).assert(hasText("private query", substring = true).not())
    }
}
