package net.havenkeys.android.ui.onboarding

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class OnboardingScreenTest {
    @get:Rule
    val rule = createComposeRule()

    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val accounts = FakeAccountRepository()

    private fun show(vm: OnboardingViewModel = OnboardingViewModel(accounts)) =
        rule.setKit { OnboardingScreen(vm, onDone = {}) }

    private fun choose(id: Int) = rule.onNode(hasText(text(id)) and hasClickAction()).performClick()
    private fun field(id: Int) = rule.onNode(hasSetTextAction() and hasText(text(id)))
    private fun typed(id: Int) =
        field(id).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text.orEmpty()

    private fun back() = rule.onNodeWithContentDescription(text(R.string.onboarding_back)).performClick()

    private fun kitPasswordVm(): OnboardingViewModel {
        accounts.kit = Outcome.Ok(KitPreview("a@example.com", "https://v.example.com"))
        return OnboardingViewModel(accounts).also {
            it.choose(OnboardingUiState.Mode.SCAN)
            it.onFrame(LumaFrame(1u, 1u, byteArrayOf(7)))
        }
    }

    @Test
    fun eachWayOpensItsStepAndBackReturns() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_server).assertExists()
        back()
        rule.onNode(hasText(text(R.string.onboarding_invite_choice)) and hasClickAction()).assertExists()
    }

    @Test
    fun signingInWaitsForAllFourAndSendsThem() {
        show()
        choose(R.string.onboarding_type_kit)
        val signIn = rule.onNode(hasText(text(R.string.onboarding_sign_in)) and hasClickAction())
        field(R.string.onboarding_server).performTextInput("https://vault.example.com")
        field(R.string.onboarding_email).performTextInput("sam@example.com")
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        signIn.assertIsNotEnabled()
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        signIn.assertIsEnabled().performClick()
        rule.waitForIdle()
        assertTrue("signIn" in accounts.calls)
    }

    @Test
    fun theSecretsAreTypedAsPasswords() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_secret_key).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        field(R.string.onboarding_master_password).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
    }

    @Test
    fun theInviteAndBothPasswordsAreTypedAsPasswords() {
        show()
        choose(R.string.onboarding_invite_choice)
        listOf(R.string.onboarding_invite, R.string.onboarding_master_password, R.string.onboarding_repeat_password)
            .forEach { field(it).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password)) }
    }

    @Test
    fun theKitPasswordIsTypedAsAPasswordUnderTheKitsAddressAndServer() {
        show(kitPasswordVm())
        field(R.string.onboarding_master_password).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        rule.onNode(hasText("a@example.com")).assertExists()
        rule.onNode(hasText("https://v.example.com")).assertExists()
    }

    @Test
    fun aNewAccountNeedsALongPasswordTypedTwice() {
        show()
        choose(R.string.onboarding_invite_choice)
        field(R.string.onboarding_invite).performTextInput("invite-token")
        field(R.string.onboarding_master_password).performTextInput("short")
        field(R.string.onboarding_master_password)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.onboarding_too_short)))
        field(R.string.onboarding_master_password).performTextInput(" but longer now")
        field(R.string.onboarding_repeat_password).performTextInput("something else")
        field(R.string.onboarding_repeat_password)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(R.string.onboarding_mismatch)))
        rule.onNode(hasText(text(R.string.onboarding_create)) and hasClickAction()).assertIsNotEnabled()
    }

    @Test
    fun aRestoreKeepsTheServerButNotTheSecrets() {
        val restoration = StateRestorationTester(rule)
        val vm = OnboardingViewModel(accounts)
        restoration.setContent { HavenTheme { OnboardingScreen(vm, onDone = {}) } }
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_server).performTextInput("https://vault.example.com")
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        restoration.emulateSavedInstanceStateRestore()
        assertEquals("https://vault.example.com", typed(R.string.onboarding_server))
        assertTrue(typed(R.string.onboarding_secret_key).isEmpty())
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
    }

    @Test
    fun aRestoreBringsNoInviteOrPasswordBack() {
        val restoration = StateRestorationTester(rule)
        val vm = OnboardingViewModel(accounts)
        restoration.setContent { HavenTheme { OnboardingScreen(vm, onDone = {}) } }
        choose(R.string.onboarding_invite_choice)
        field(R.string.onboarding_invite).performTextInput("invite-token")
        field(R.string.onboarding_master_password).performTextInput("long enough password")
        field(R.string.onboarding_repeat_password).performTextInput("long enough password")
        restoration.emulateSavedInstanceStateRestore()
        listOf(R.string.onboarding_invite, R.string.onboarding_master_password, R.string.onboarding_repeat_password)
            .forEach { assertTrue(typed(it).isEmpty()) }
    }

    @Test
    fun aRestoreBringsNoKitPasswordBack() {
        val restoration = StateRestorationTester(rule)
        val vm = kitPasswordVm()
        restoration.setContent { HavenTheme { OnboardingScreen(vm, onDone = {}) } }
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        restoration.emulateSavedInstanceStateRestore()
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
    }

    /** What the user sees: a step opened again starts empty (each opening is a fresh composition). */
    @Test
    fun aStepOpenedAgainStartsEmpty() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        back()
        choose(R.string.onboarding_type_kit)
        assertTrue(typed(R.string.onboarding_secret_key).isEmpty())
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
        back()
        choose(R.string.onboarding_invite_choice)
        field(R.string.onboarding_invite).performTextInput("invite-token")
        field(R.string.onboarding_master_password).performTextInput("long enough password")
        back()
        choose(R.string.onboarding_invite_choice)
        assertTrue(typed(R.string.onboarding_invite).isEmpty())
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
    }

    @Test
    fun theSecretsAreEmptiedAsTheyAreSent() {
        show()
        choose(R.string.onboarding_type_kit)
        field(R.string.onboarding_server).performTextInput("https://vault.example.com")
        field(R.string.onboarding_email).performTextInput("sam@example.com")
        field(R.string.onboarding_secret_key).performTextInput("A3-KEY")
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        rule.onNode(hasText(text(R.string.onboarding_sign_in)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertTrue("signIn" in accounts.calls)
        // Ruling (final review): the password is emptied as it is sent; the Secret Key stays for another attempt.
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
        assertEquals("A3-KEY", typed(R.string.onboarding_secret_key))
    }

    /** Final review (stage 4): a failed activation keeps the invite and empties both passwords. */
    @Test
    fun activatingEmptiesThePasswordsAndKeepsTheInvite() {
        show()
        choose(R.string.onboarding_invite_choice)
        field(R.string.onboarding_invite).performTextInput("invite-token")
        field(R.string.onboarding_master_password).performTextInput("long enough password")
        field(R.string.onboarding_repeat_password).performTextInput("long enough password")
        rule.onNode(hasText(text(R.string.onboarding_create)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertTrue("activate" in accounts.calls)
        assertEquals("invite-token", typed(R.string.onboarding_invite))
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
        assertTrue(typed(R.string.onboarding_repeat_password).isEmpty())
    }

    /** Final review (stage 4): pins the clear itself, not only that a new step starts empty. */
    @Test
    fun aSecretLeavingTheCompositionIsEmptied() {
        var shown by mutableStateOf(true)
        var secret: Secret? = null
        rule.setKit { if (shown) secret = rememberSecret() }
        rule.runOnIdle {
            secret!!.text.edit { append("A3-KEY") }
            secret!!.shown = true
        }
        shown = false
        rule.waitForIdle()
        assertTrue(secret!!.text.text.isEmpty())
        assertFalse(secret!!.shown)
    }

    @Test
    fun theKitPasswordIsEmptiedAsItIsSent() {
        show(kitPasswordVm())
        field(R.string.onboarding_master_password).performTextInput("correct horse")
        rule.onNode(hasText(text(R.string.onboarding_sign_in)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertTrue("signInWithKit" in accounts.calls)
        assertTrue(typed(R.string.onboarding_master_password).isEmpty())
    }
}
