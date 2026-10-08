package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.isSelected
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.trash.trashSummary
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class SettingsScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val settings = FakeSettingsRepository()
    private val accounts = FakeAccountRepository()
    private val vault = FakeVaultRepository()
    private val done = mutableListOf<String>()

    private var verified = true

    private fun show(online: Boolean = true, enrolled: Boolean = false) {
        val vm = SettingsViewModel(settings, accounts, vault, VaultEventsHub(), biometricEnrolled = { enrolled })
        val actions = SettingsActions(
            biometricAvailable = true,
            forgetBiometric = { done += "forget" },
            enrollBiometric = { password ->
                done += "enroll:$password"
                Outcome.Ok(Unit)
            },
            verifyUser = { _ ->
                done += "verify"
                verified
            },
        )
        val navigation = SettingsNavigation(
            onDevices = { done += "devices" },
            onAutofillSetup = { done += "setup" },
            onPairing = { done += "pairing" },
            onTrash = { done += "trash" },
        )
        rule.setKit { SettingsScreen(vm, online, actions, navigation, PaddingValues()) }
    }

    private fun row(text: String) = rule.onNode(hasText(text) and hasClickAction())

    private fun switch(text: String) = rule.onNode(hasText(text) and hasRole(Role.Switch))

    /** A dialog's button; its row behind the dialog has the same words. */
    private fun dialogButton(text: String) =
        rule.onNode(hasText(text) and hasRole(Role.Button) and hasAnyAncestor(isDialog()))

    @Test
    fun everySettingIsAGroupedRow() {
        show()
        listOf("Settings", "Security", "Autofill", "Account").forEach { rule.onNodeWithText(it).assert(isHeading()) }
        row("Lock automatically").assert(hasText("After 15 minutes"))
        row("Clear copied items").assert(hasText("After 30 seconds"))
        switch("Lock when the screen turns off").assertIsOn()
        switch("Unlock with fingerprint or face").assertIsOff()
        switch("Confirm before filling").assertIsOff()
        switch("Check website–app links").assertIsOn()
        rule.onNodeWithText("user@example.com").assertExists()
        row("Autofill setup").performClick()
        row("Devices").performClick()
        assertEquals(listOf("setup", "devices"), done)
    }

    @Test
    fun theTrashRowShowsItsCountAndOpensTheTrash() {
        vault.trashList = Outcome.Ok(
            listOf(trashSummary("a", "A"), trashSummary("b", "B")),
        )
        show()
        rule.onNodeWithText("Vault").assert(isHeading())
        row("Trash").assert(hasText("2")).performClick()
        assertEquals(listOf("trash"), done)
    }

    @Test
    fun aSwitchSavesTheSetting() {
        show()
        switch("Confirm before filling").performClick()
        assertEquals(true, settings.updates.last().confirmBeforeFilling)
        switch("Confirm before filling").assertIsOn()
    }

    @Test
    fun aChoiceOpensASheetAndSavesThePick() {
        show()
        row("Lock automatically").performClick()
        rule.onNode(isDialog()).assertExists()
        rule.onNode(hasText("After 15 minutes") and hasRole(Role.RadioButton)).assert(isSelected())
        rule.onNode(hasText("After 5 minutes") and hasRole(Role.RadioButton)).performClick()
        assertEquals(5u, settings.updates.last().autoLockMinutes)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun turningBiometricsOffForgetsTheKey() {
        show(enrolled = true)
        switch("Unlock with fingerprint or face").assertIsOn().performClick()
        assertEquals(listOf("forget"), done)
    }

    @Test
    fun turningBiometricsOnAsksForTheMasterPasswordInADialog() {
        show()
        switch("Unlock with fingerprint or face").performClick()
        dialogButton("Turn on").assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("correct horse")
        dialogButton("Turn on").performClick()
        rule.waitForIdle()
        assertEquals(listOf("enroll:correct horse"), done)
        rule.onNode(isDialog()).assertDoesNotExist()
    }

    @Test
    fun signingOutAsksFirst() {
        show()
        row("Sign out and lock").performClick()
        dialogButton("Sign out and lock").performClick()
        assertTrue("signOut" in accounts.calls)
    }

    @Test
    fun removingTheDeviceShowsRustsAnswerAndCanTryAgain() {
        accounts.done = Outcome.Failed("invalid_input")
        show()
        row("Remove this device").performClick()
        val remove = dialogButton("Remove this device")
        remove.assertIsNotEnabled()
        rule.onNode(hasSetTextAction()).performTextInput("someone@example.com")
        remove.performClick()
        // The kit shows the error under the field and announces it as the field's error.
        rule.onNode(hasSetTextAction()).assert(
            SemanticsMatcher.expectValue(SemanticsProperties.Error, "That is not this account’s email."),
        )
        accounts.done = Outcome.Ok(Unit)
        remove.performClick()
        assertEquals(
            listOf("removeDevice:someone@example.com", "removeDevice:someone@example.com"),
            accounts.calls.filter { it.startsWith("removeDevice") },
        )
    }

    @Test
    fun theSameRemovalErrorTwiceCanStillBeRetried() {
        accounts.done = Outcome.Failed("invalid_input")
        show()
        row("Remove this device").performClick()
        rule.onNode(hasSetTextAction()).performTextInput("someone@example.com")
        repeat(3) {
            dialogButton("Remove this device").performClick()
            rule.waitForIdle()
        }
        assertEquals(3, accounts.calls.count { it.startsWith("removeDevice") })
    }

    @Test
    fun deletingTheAccountExplainsThenAsksForEmailAndPassword() {
        show()
        row("Delete account and all data").performClick()
        rule.onNodeWithText(
            "This deletes your vault from the server, signs out every device, and erases this phone’s copy. " +
                "Neither you nor the server’s operator can recover it. To keep a copy, export an encrypted backup " +
                "from the desktop first.",
        ).assertExists()
        dialogButton("Delete anyway").performClick()
        val delete = dialogButton("Delete account")
        delete.assertIsNotEnabled()
        val fields = rule.onAllNodes(hasSetTextAction() and hasAnyAncestor(isDialog()))
        fields[0].performTextInput("user@example.com")
        delete.assertIsNotEnabled()
        fields[1].performTextInput("pw")
        delete.performClick()
        rule.waitForIdle()
        assertEquals(listOf("verify"), done.filter { it == "verify" })
        assertEquals(listOf("deleteAccount:user@example.com"), accounts.calls.filter { it.startsWith("delete") })
    }

    @Test
    fun aFailedCheckThatItIsTheUserDeletesNothing() {
        verified = false
        show()
        row("Delete account and all data").performClick()
        dialogButton("Delete anyway").performClick()
        val fields = rule.onAllNodes(hasSetTextAction() and hasAnyAncestor(isDialog()))
        fields[0].performTextInput("user@example.com")
        fields[1].performTextInput("pw")
        dialogButton("Delete account").performClick()
        rule.waitForIdle()
        assertTrue(accounts.calls.none { it.startsWith("delete") })
    }

    @Test
    fun offlineAccountRowsSaySo() {
        show(online = false)
        row("Sign out and lock").assert(hasText("HavenKeys is offline — the vault is read-only until it reconnects."))
    }
}
