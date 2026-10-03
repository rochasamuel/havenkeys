package net.havenkeys.android.ui.autofillsetup

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.setKit
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class AutofillSetupScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Test
    fun whenAutofillIsOffItOffersSettingsAndExplainsChrome() {
        rule.setKit { AutofillSetupScreen(online = false, onBack = {}, onLock = {}) }
        rule.onNode(hasText(text(R.string.autofill_setup_title)) and isHeading()).assertIsDisplayed()
        rule.onNodeWithText(text(R.string.autofill_setup_off)).assertExists()
        rule.onNode(hasText(text(R.string.autofill_setup_open)) and hasClickAction()).assertExists()
        rule.onNode(hasText(text(R.string.autofill_setup_chrome_title)) and isHeading()).assertExists()
        rule.onNodeWithText(text(R.string.vault_offline)).assertExists()
    }
}
