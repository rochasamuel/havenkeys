package net.havenkeys.android.ui.unlock

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
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.setKit
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
        assertTrue(typed(R.string.unlock_secret_key).isEmpty())
    }

    @Test
    fun aFailedUnlockIsTheFieldsError() {
        show(UnlockUiState(errorCode = "unlock_failed"))
        field(R.string.unlock_password_hint)
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, text(errorText("unlock_failed"))))
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
