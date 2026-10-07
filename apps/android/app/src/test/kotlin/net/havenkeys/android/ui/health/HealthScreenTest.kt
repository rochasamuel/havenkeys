package net.havenkeys.android.ui.health

import android.content.Intent
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CompletableDeferred
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.HealthCountsView
import uniffi.havenkeys_mobile.HealthIssueView
import uniffi.havenkeys_mobile.HealthKind
import uniffi.havenkeys_mobile.HealthView
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2400dp")
class HealthScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(
            listOf(
                ItemSummary("a", ItemKind.LOGIN, "GitHub", "sam@example.com", null, false, false, 0, 0),
                ItemSummary("b", ItemKind.LOGIN, "Bank", null, null, false, false, 0, 0),
            ),
        )
        healthView = Outcome.Ok(
            HealthView(
                HealthCountsView(1u, 0u, 1u, 1u, 0u, 0u, 0u),
                listOf(
                    HealthIssueView("a", listOf(HealthKind.WEAK, HealthKind.OLD), null, null, false),
                    HealthIssueView("b", listOf(HealthKind.PASSKEY), null, null, false),
                ),
            ),
        )
    }
    private val opened = mutableListOf<String>()
    private val edited = mutableListOf<String>()

    private fun show(online: Boolean = true) {
        val vm = HealthViewModel(vault, VaultEventsHub())
        rule.setKit {
            HealthScreen(vm, online, HealthNavigation({ opened += it }, { edited += it }, {}, {}))
        }
    }

    private fun more(title: String) = rule.onNodeWithContentDescription("Actions for $title")

    @Test
    fun everyCheckShowsItsCountAndWhyItMatters() {
        show()
        rule.onNode(hasText("Weak passwords") and hasClickAction())
            .assertExists()
        rule.onNodeWithText("Easy to guess. Generate a strong password instead.").assertExists()
        listOf(
            "Reused passwords", "Unsecured websites", "Duplicates", "Passkeys available",
            "Two-factor authentication", "Old passwords",
        ).forEach { rule.onNodeWithText(it).assertExists() }
    }

    @Test
    fun aRowShowsItsChipsAndOpensTheLogin() {
        show()
        rule.onNodeWithText("Weak password").assertExists()
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        assertEquals(listOf("a"), opened)
    }

    @Test
    fun aWeakPasswordOffersChangePasswordAndDismiss() {
        show()
        more("GitHub").performClick()
        rule.onNodeWithText("Change password").performClick()
        assertEquals(listOf("a"), edited)
        more("GitHub").performClick()
        rule.onNodeWithText("Dismiss: Weak password").performClick()
        rule.waitForIdle()
        assertTrue("ignore:a:WEAK" in vault.calls)
    }

    @Test
    fun howToEnableShowsOnlyWhenRustHasALink() {
        show()
        more("Bank").performClick()
        rule.onNodeWithText("How to enable").assertDoesNotExist()
    }

    @Test
    fun howToEnableIsOfferedWithRustsLink() {
        vault.helpUrl = Outcome.Ok("https://bank.example/passkeys")
        show()
        more("Bank").performClick()
        rule.onNodeWithText("How to enable").assertExists()
        assertTrue("help:b:PASSKEY" in vault.calls)
    }

    @Test
    fun howToEnableOpensRustsLinkUnchanged() {
        val url = "https://bank.example/help?topic=passkeys#setup"
        vault.helpUrl = Outcome.Ok(url)
        show()
        more("Bank").performClick()
        rule.onNodeWithText("How to enable").performClick()
        rule.waitForIdle()
        val started = Shadows.shadowOf(RuntimeEnvironment.getApplication()).nextStartedActivity
        assertEquals(Intent.ACTION_VIEW, started.action)
        assertEquals(url, started.dataString)
    }

    @Test
    fun offlineNothingThatWritesIsOffered() {
        show(online = false)
        more("GitHub").performClick()
        rule.onNodeWithText("Open").assertExists()
        rule.onNodeWithText("Change password").assertDoesNotExist()
        rule.onNodeWithText("Dismiss: Weak password").assertDoesNotExist()
    }

    @Test
    fun aLoginWhoseOnlyCheckWasDismissedLeavesAtOnce() {
        show()
        vault.healthGate = CompletableDeferred()
        more("Bank").performClick()
        rule.onNodeWithText("Dismiss: Supports passkeys").performClick()
        rule.waitForIdle()
        rule.onNode(hasText("Bank") and hasClickAction()).assertDoesNotExist()
    }

    @Test
    fun aCheckShowsOnlyItsLoginsAndAllIssuesComesBack() {
        show()
        rule.onNode(hasText("Passkeys available") and hasClickAction()).performClick()
        rule.onNode(hasText("GitHub") and hasClickAction()).assertDoesNotExist()
        rule.onNode(hasText("Bank") and hasClickAction()).assertExists()
        rule.onNode(hasText("All issues") and hasRole(Role.Button)).performClick()
        rule.onNode(hasText("GitHub") and hasClickAction()).assertExists()
        rule.onNodeWithText("Dismissed").performClick()
        rule.onNodeWithText("Nothing here.").assertExists()
    }

    @Test
    fun underAllIssuesTheSegmentAloneNamesTheList() {
        show()
        rule.onAllNodesWithText("All issues").assertCountEquals(1)
    }

    @Test
    fun noIssuesSaysSoAndNeverThatTheVaultIsSecure() {
        vault.healthView = Outcome.Ok(HealthView(HealthCountsView(0u, 0u, 0u, 0u, 0u, 0u, 0u), emptyList()))
        show()
        rule.onNodeWithText("No issues found.").assertExists()
        rule.onNodeWithText("is secure", substring = true, ignoreCase = true).assertDoesNotExist()
    }

    @Test
    fun theRowsActionsWaitWhileItsSaveIsInFlight() {
        show()
        val saving = CompletableDeferred<Unit>()
        vault.ignoreGate = saving
        vault.healthGate = CompletableDeferred()
        more("GitHub").performClick()
        rule.onNodeWithText("Dismiss: Weak password").performClick()
        rule.waitForIdle()
        rule.onNodeWithText("Weak password").assertDoesNotExist()
        more("GitHub").assertIsNotEnabled()
        saving.complete(Unit)
        rule.waitForIdle()
        more("GitHub").assertIsEnabled()
    }
}
