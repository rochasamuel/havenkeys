package net.havenkeys.android.ui.unlock

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class UnlockScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val submitted = mutableListOf<Pair<String, String?>>()
    private var biometrics = 0

    private fun show(state: UnlockUiState = UnlockUiState()) = rule.setKit {
        UnlockForm(state, onSubmit = { p, k -> submitted += p to k }, onBiometric = { biometrics++ })
    }

    private fun field(label: Int) = rule.onNode(hasSetTextAction() and hasText(text(label)))
    private fun unlock() = rule.onNode(hasText(text(R.string.unlock_button)) and hasClickAction())
    private fun typed(label: Int) =
        field(label).fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text.orEmpty()

    /** Review (stage 4): the desktop's headline sets "locked" in the serif italic; the phone's had lost it. */
    @Test
    fun theHeadlineSetsLockedInItalic() {
        show()
        val title = rule.onNodeWithText(text(R.string.unlock_title)).fetchSemanticsNode()
        val shown = title.config[SemanticsProperties.Text].single()
        val word = text(R.string.unlock_title_locked)
        val italic = shown.spanStyles.single { it.item.fontStyle == FontStyle.Italic }
        assertEquals(word, shown.text.substring(italic.start, italic.end))
    }

    /** Review (stage 5): unlock sat on a 20dp gutter, every other screen on the 16dp one. */
    @Test
    fun unlockSitsOnTheSameGutterAsEveryOtherScreen() {
        show()
        val button = unlock().fetchSemanticsNode().boundsInRoot
        val root = rule.onRoot().fetchSemanticsNode().boundsInRoot
        val gutter = HavenSpacing.gutter.value * rule.density.density
        assertEquals(gutter, button.left - root.left, 1f)
        assertEquals(gutter, root.right - button.right, 1f)
    }

    @Test
    fun theMasterPasswordIsAPasswordFieldAndUnlockWaitsForIt() {
        show()
        field(R.string.unlock_password_hint).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        unlock().assertIsNotEnabled()
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().assertIsEnabled()
    }

    @Test
    fun submittingSendsThePasswordOnceAndEmptiesTheField() {
        show()
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().performClick()
        assertEquals(listOf("hunter2" to null), submitted)
        assertTrue(typed(R.string.unlock_password_hint).isEmpty())
        unlock().assertIsNotEnabled()
    }

    @Test
    fun aDeviceWithoutItsSecretKeyAsksForBoth() {
        show(UnlockUiState(needsSecretKey = true))
        field(R.string.unlock_secret_key).assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        unlock().assertIsNotEnabled()
        field(R.string.unlock_secret_key).performTextInput("A3-KEY")
        unlock().performClick()
        assertEquals(listOf("hunter2" to "A3-KEY"), submitted)
        // Ruling (final review): the password is emptied on submit; the Secret Key stays for another attempt.
        assertTrue(typed(R.string.unlock_password_hint).isEmpty())
        assertEquals("A3-KEY", typed(R.string.unlock_secret_key))
    }

    /** Final review (stage 4): the kept Secret Key is emptied once the vault opens. */
    @Test
    fun theSecretKeyIsEmptiedOnceTheVaultOpens() {
        var state by mutableStateOf(UnlockUiState(needsSecretKey = true))
        rule.setKit { UnlockForm(state, onSubmit = { p, k -> submitted += p to k }, onBiometric = {}) }
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        field(R.string.unlock_secret_key).performTextInput("A3-KEY")
        unlock().performClick()
        state = state.copy(errorCode = "unlock_failed")
        rule.waitForIdle()
        assertEquals("A3-KEY", typed(R.string.unlock_secret_key))
        state = state.copy(errorCode = null, unlocked = true)
        rule.waitForIdle()
        assertTrue(typed(R.string.unlock_secret_key).isEmpty())
    }

    /** Final review (stage 4): leaving the screen empties both fields, not only dropping them. */
    @Test
    fun leavingTheScreenEmptiesBothFields() {
        var shown by mutableStateOf(true)
        val password = TextFieldState()
        val secretKey = TextFieldState()
        rule.setKit {
            if (shown) UnlockForm(UnlockUiState(), { _, _ -> }, {}, password = password, secretKey = secretKey)
        }
        password.edit { append("hunter2") }
        secretKey.edit { append("A3-KEY") }
        shown = false
        rule.waitForIdle()
        assertTrue(password.text.isEmpty())
        assertTrue(secretKey.text.isEmpty())
    }

    @Test
    fun aFailedUnlockIsTheFieldsError() {
        show(UnlockUiState(errorCode = "unlock_failed"))
        field(R.string.unlock_password_hint)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(errorText("unlock_failed"))))
    }

    /** Final review (stage 4): only a refused password is the field's error; the rest is a line under the form. */
    @Test
    fun otherFailuresAreALineNotThePasswordFieldsError() {
        var state by mutableStateOf(UnlockUiState())
        rule.setKit { UnlockForm(state, onSubmit = { _, _ -> }, onBiometric = {}) }
        for (code in listOf("offline", "biometric_unavailable", "secret_key_required", "keychain_unavailable")) {
            state = UnlockUiState(errorCode = code)
            rule.waitForIdle()
            field(R.string.unlock_password_hint).assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Error))
            rule.onNodeWithText(text(errorText(code))).assertIsDisplayed()
        }
    }

    @Test
    fun aFailedUnlockIsNotAlsoALine() {
        show(UnlockUiState(errorCode = "unlock_failed"))
        rule.onAllNodesWithText(text(errorText("unlock_failed"))).assertCountEquals(0)
    }

    @Test
    fun withoutBiometricsSetUpThereIsNoBiometricButton() {
        show(UnlockUiState(offerBiometric = false))
        rule.onNodeWithText(text(R.string.unlock_biometric)).assertDoesNotExist()
    }

    @Test
    fun whileBusyNothingCanBeSentTwice() {
        show(UnlockUiState(busy = true, offerBiometric = true))
        rule.onNode(hasText(text(R.string.unlock_busy)) and hasClickAction()).assertIsNotEnabled()
        rule.onNode(hasText(text(R.string.unlock_biometric)) and hasClickAction()).assertIsNotEnabled()
    }

    @Test
    fun biometricsIsOfferedOnlyWhenSetUp() {
        show(UnlockUiState(offerBiometric = true))
        rule.onNodeWithText(text(R.string.unlock_biometric)).performClick()
        assertEquals(1, biometrics)
    }

    @Test
    fun aRestoredUnlockScreenBringsNoPasswordBack() {
        val restoration = StateRestorationTester(rule)
        restoration.setContent { HavenTheme { UnlockForm(UnlockUiState(), { _, _ -> }, {}) } }
        field(R.string.unlock_password_hint).performTextInput("hunter2")
        restoration.emulateSavedInstanceStateRestore()
        assertTrue(typed(R.string.unlock_password_hint).isEmpty())
    }
}
