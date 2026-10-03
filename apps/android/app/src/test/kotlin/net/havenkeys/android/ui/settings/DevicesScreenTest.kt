package net.havenkeys.android.ui.settings

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.DeviceInfo

@RunWith(RobolectricTestRunner::class)
class DevicesScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    private val accounts = FakeAccountRepository().apply {
        deviceList = Outcome.Ok(
            listOf(
                DeviceInfo("d1", "Pixel 8", "2026-01-01T00:00:00Z", null, true),
                DeviceInfo("d2", "Work laptop", "2026-01-01T00:00:00Z", "2026-10-03T10:00:00Z", false),
            ),
        )
    }

    private fun show() {
        val vm = DevicesViewModel(accounts, VaultEventsHub())
        rule.setKit { DevicesScreen(vm, online = true, onBack = {}, onLock = {}) }
    }

    @Test
    fun eachDeviceIsARowAndThisPhoneIsMarked() {
        show()
        rule.onNodeWithText(text(R.string.devices_this_phone)).assertExists()
        rule.onNode(hasText("Work laptop")).assertExists()
    }

    @Test
    fun revokingAsksFirstAndEndsThatSession() {
        show()
        rule.onAllNodesWithText(text(R.string.devices_revoke))[1].performClick()
        rule.onNodeWithText(text(R.string.devices_revoke_confirm, "Work laptop")).assertExists()
        rule.onNode(
            hasText(text(R.string.devices_revoke)) and hasAnyAncestor(isDialog()) and hasClickAction(),
        ).performClick()
        rule.waitForIdle()
        assertTrue("revoke:d2" in accounts.calls)
    }

    @Test
    fun cancelEndsNothing() {
        show()
        rule.onAllNodesWithText(text(R.string.devices_revoke))[0].performClick()
        rule.onNodeWithText(text(R.string.devices_revoke_this)).assertExists()
        rule.onNodeWithText(text(R.string.settings_cancel)).performClick()
        rule.waitForIdle()
        assertTrue(accounts.calls.none { it.startsWith("revoke") })
    }

    /** Final review (stage 4): every row's button read just "Revoke"; TalkBack now hears which device. */
    @Test
    fun eachRevokeButtonNamesItsDevice() {
        show()
        rule.onNode(hasContentDescription(text(R.string.devices_revoke_named, "Work laptop")) and hasClickAction())
            .assert(hasRole(Role.Button))
            .performClick()
        rule.onNodeWithText(text(R.string.devices_revoke_confirm, "Work laptop")).assertExists()
    }
}
