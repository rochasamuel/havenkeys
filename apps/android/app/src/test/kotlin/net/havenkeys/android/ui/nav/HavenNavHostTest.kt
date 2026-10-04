package net.havenkeys.android.ui.nav

import androidx.activity.ComponentActivity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.navigation.NavHostController
import androidx.navigation.compose.rememberNavController
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.fakes.status
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.removeAnimations
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.LockState

/** The real app NavHost and shell over fakes; Unlock is a stand-in button that unlocks the fake vault. */
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h891dp")
class HavenNavHostTest {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val github = ItemSummary("1", ItemKind.LOGIN, "GitHub", "sam", null, false, false, 0, 0)
    private val events = VaultEventsHub()
    private val vault = FakeVaultRepository().apply {
        items = Outcome.Ok(listOf(github))
        recent = Outcome.Ok(listOf(github))
        view = Outcome.Ok(ItemView(github, emptyList()))
    }
    private lateinit var nav: NavHostController

    private val services = NavServices(
        vault = vault,
        accounts = FakeAccountRepository(),
        settings = FakeSettingsRepository(),
        events = events,
        clipboard = SensitiveClipboard(RuntimeEnvironment.getApplication(), CoroutineScope(SupervisorJob())),
        hasBiometricUnlock = { false },
        unlockScreen = { onUnlocked ->
            HavenButton(
                UNLOCK,
                onClick = {
                    vault.nextStatus = Outcome.Ok(status())
                    events.unlocked()
                    onUnlocked()
                },
            )
        },
        settingsActions = { SettingsActions(false, {}, { Outcome.Ok(Unit) }) },
        canVerifyUser = { true },
        verifyUser = { _, _ -> true },
    )

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Before
    fun show() {
        removeAnimations()
        rule.setContent {
            HavenTheme {
                nav = rememberNavController()
                HavenNavHost(services, navController = nav)
            }
        }
        rule.waitForIdle()
    }

    @After
    fun animationsBack() = removeAnimations(false)

    private fun lock() {
        vault.nextStatus = Outcome.Ok(status(LockState.LOCKED))
        events.locked("user")
        rule.waitForIdle()
    }

    private fun assertUnlockWithNothingBehind() {
        rule.onNodeWithText(UNLOCK).assertIsDisplayed()
        rule.runOnIdle {
            assertEquals(Routes.UNLOCK, nav.currentDestination?.route)
            assertNull(nav.previousBackStackEntry)
        }
    }

    private fun back() {
        rule.runOnIdle { rule.activity.onBackPressedDispatcher.onBackPressed() }
        rule.waitForIdle()
    }

    private fun openLogins() {
        rule.onNode(hasText(text(R.string.tab_items)) and hasRole(Role.Tab)).performClick()
        rule.onNode(hasText(text(Category.LOGINS.label)) and hasClickAction()).performClick()
        rule.onNode(hasText(text(Category.LOGINS.label)) and isHeading()).assertIsDisplayed()
    }

    @Test
    fun lockingFromAnItemLeavesOnlyUnlock() {
        rule.onNode(hasText("GitHub") and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(Routes.ITEM, nav.currentDestination?.route) }
        lock()
        assertUnlockWithNothingBehind()
    }

    @Test
    fun lockingFromSearchLeavesOnlyUnlock() {
        rule.onNode(hasText(text(R.string.shell_search)) and hasClickAction()).performClick()
        rule.onNode(hasSetTextAction()).performTextInput("git")
        rule.runOnIdle { assertEquals(Routes.SEARCH, nav.currentDestination?.route) }
        lock()
        assertUnlockWithNothingBehind()
    }

    @Test
    fun lockingFromACategoryListLeavesOnlyUnlockAndUnlockingOpensAFreshHome() {
        openLogins()
        lock()
        assertUnlockWithNothingBehind()
        rule.onNodeWithText(UNLOCK).performClick()
        rule.waitForIdle()
        rule.onNode(hasText(text(R.string.home_recent)) and isHeading()).assertIsDisplayed()
        rule.onAllNodes(hasText(text(Category.LOGINS.label)) and isHeading()).assertCountEquals(0)
        rule.runOnIdle {
            assertEquals(Routes.SHELL, nav.currentDestination?.route)
            assertNull(nav.previousBackStackEntry)
        }
    }

    @Test
    fun backFromACategoryGoesToTheItemsRootThenHome() {
        openLogins()
        back()
        rule.onNode(hasText(text(R.string.tab_items)) and isHeading()).assertIsDisplayed()
        back()
        rule.onNode(hasText(text(R.string.home_recent)) and isHeading()).assertIsDisplayed()
    }

    private companion object {
        const val UNLOCK = "Unlock (test)"
    }
}
