package net.havenkeys.android.ui.trash

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isDialog
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class TrashScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val now = System.currentTimeMillis()
    private val day = 86_400_000L
    private val vault = FakeVaultRepository().apply {
        trashList = Outcome.Ok(
            listOf(
                trashSummary("a", "GitHub", daysLeft = 27, passkey = true, trashedAt = now - 3 * day - 1_000),
                trashSummary("b", "Bank", daysLeft = 0, trashedAt = now - 1_000),
            ),
        )
    }

    private fun show(online: Boolean = true) {
        val vm = TrashViewModel(vault, VaultEventsHub())
        rule.setKit { TrashScreen(vm, online, onBack = {}, onLock = {}) }
    }

    private fun button(text: String) = rule.onNode(hasText(text) and hasRole(Role.Button))

    private fun more() = rule.onNodeWithContentDescription("More")

    private fun dialogButton(text: String) =
        rule.onNode(hasText(text) and hasRole(Role.Button) and hasAnyAncestor(isDialog()))

    @Test
    fun theRowsShowWhenEachWasDeletedAndWhenItGoes() {
        show()
        rule.onNodeWithText("Items here are removed for good after 30 days.").assertExists()
        rule.onNodeWithText("GitHub").assertExists()
        rule.onNodeWithText("Deleted 3 days ago · Removed in 27 days").assertExists()
        rule.onNodeWithText("Deleted today · Removed at next sync").assertExists()
    }

    @Test
    fun anEmptyTrashSaysSoAndCannotBeEmptied() {
        vault.trashList = Outcome.Ok(emptyList())
        show()
        rule.onNodeWithText("Trash is empty.").assertExists()
        more().assertIsNotEnabled()
    }

    @Test
    fun emptyTrashAsksFirstWithTheSameVerbOnItsButton() {
        show()
        more().performClick()
        button("Empty Trash").assertTouchTarget().performClick()
        rule.onNodeWithText("Delete 2 items permanently? This cannot be undone.").assertExists()
        dialogButton("Delete permanently").performClick()
        rule.waitForIdle()
        assertTrue("emptyTrash" in vault.calls)
    }

    @Test
    fun aRowOpensASheetWithItsOverviewAndNoValue() {
        show()
        rule.onNodeWithText("GitHub").performClick()
        rule.onNodeWithText("Restore this item to see or use its passwords and codes.").assertExists()
        dialogButton("Restore").assertIsEnabled()
        dialogButton("Delete permanently").assertIsEnabled()
        // Overview only: nothing of the item is read from Rust.
        assertTrue(vault.calls.none { it.startsWith("reveal") || it == "totp" })
    }

    @Test
    fun restoreFromTheSheetPutsTheItemBack() {
        show()
        rule.onNodeWithText("GitHub").performClick()
        dialogButton("Restore").performClick()
        rule.waitForIdle()
        assertTrue("restore:a" in vault.calls)
    }

    @Test
    fun deletePermanentlyAsksFirstAndWarnsOfPasskeys() {
        show()
        rule.onNodeWithText("GitHub").performClick()
        dialogButton("Delete permanently").performClick()
        rule.onNodeWithText("Delete “GitHub” permanently? This cannot be undone.").assertExists()
        rule.onNodeWithText("Its passkeys go too. Sites that use them will need another way to sign in.")
            .assertExists()
        assertTrue(vault.calls.none { it.startsWith("purge") })
        // The sheet's button opened the dialog, whose window is the last one.
        rule.onAllNodes(hasText("Delete permanently") and hasRole(Role.Button)).onLast().performClick()
        rule.waitForIdle()
        assertEquals(1, vault.calls.count { it == "purge:a" })
    }

    @Test
    fun offlineEveryActionWaits() {
        show(online = false)
        more().assertIsNotEnabled()
        rule.onNodeWithText("GitHub").performClick()
        dialogButton("Restore").assertIsNotEnabled()
        dialogButton("Delete permanently").assertIsNotEnabled()
    }
}
