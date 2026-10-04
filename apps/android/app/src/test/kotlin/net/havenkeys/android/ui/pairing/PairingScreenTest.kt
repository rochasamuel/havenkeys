package net.havenkeys.android.ui.pairing

import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.PairingRequestView

@RunWith(RobolectricTestRunner::class)
class PairingScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    private val link = "havenkeys://pair/v1?x"
    private val accounts = FakeAccountRepository().apply {
        scannedLink = Outcome.Ok(link)
        request = Outcome.Ok(PairingRequestView(link, "Desktop · Linux", "187.1.2.3", "São Paulo, BR", "not a date"))
    }

    // The camera is not composed in tests: the ViewModel is driven to CONFIRM before the screen shows.
    private fun show() {
        val vm = PairingViewModel(accounts)
        vm.onFrame(LumaFrame(1u, 1u, byteArrayOf(1)))
        rule.setKit {
            PairingScreen(
                vm,
                online = true,
                canVerify = { true },
                verifyUser = { _, _ -> false },
                onBack = {},
                onLock = {},
            )
        }
        rule.waitForIdle()
    }

    @Test
    fun theRequestShowsWhoIsAsking() {
        show()
        rule.onNodeWithText("Desktop · Linux").assertExists()
        rule.onNode(hasText("São Paulo, BR", substring = true) and hasText("187.1.2.3", substring = true))
            .assertExists()
    }

    @Test
    fun allowThatFailsTheBiometricCheckApprovesNothing() {
        show()
        rule.onNode(hasText(text(R.string.pairing_allow)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertTrue(accounts.approved.isEmpty())
    }

    @Test
    fun denyDeniesTheCode() {
        show()
        rule.onNode(hasText(text(R.string.pairing_deny)) and hasClickAction()).performClick()
        rule.waitForIdle()
        assertEquals(listOf(link), accounts.denied)
    }
}
