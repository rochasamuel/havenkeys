package net.havenkeys.android.ui.item

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import uniffi.havenkeys_mobile.TotpNow

@RunWith(RobolectricTestRunner::class)
class DetailRowsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int, vararg args: Any) = RuntimeEnvironment.getApplication().getString(id, *args)

    @Test
    fun aShownValueIsOneStopWithItsCopy() {
        var copies = 0
        rule.setKit { InsetGroup { row { ShownRow("Username", "sam@example.com", onCopy = { copies++ }) } } }
        rule.onNode(hasText("Username") and hasText("sam@example.com")).assertExists()
        rule.onNodeWithContentDescription(text(R.string.copy, "Username")).assertTouchTarget().performClick()
        assertEquals(1, copies)
    }

    @Test
    fun aSecretShowsTheMaskUntilRevealed() {
        rule.setKit { InsetGroup { row { SecretRow("Password", revealed = null, onReveal = {}, onCopy = {}) } } }
        rule.onNode(hasText("Password") and hasContentDescription(text(R.string.hidden_value))).assertExists()
        rule.onNodeWithContentDescription(text(R.string.reveal, "Password")).assert(hasRole(Role.Button))
        rule.onAllNodesWithText("hunter2").assertCountEquals(0)
    }

    /** Final review (stage 4): TalkBack read "Password, Hidden Password"; the label is now read once. */
    @Test
    fun aMaskedSecretReadsItsLabelOnce() {
        rule.setKit { InsetGroup { row { SecretRow("Password", revealed = null, onReveal = {}, onCopy = {}) } } }
        val row = rule.onNode(hasText("Password") and hasContentDescription(text(R.string.hidden_value)))
            .fetchSemanticsNode().config
        val spoken = row.getOrElse(SemanticsProperties.Text) { emptyList() }.map { it.text } +
            row.getOrElse(SemanticsProperties.ContentDescription) { emptyList() }
        assertEquals(1, spoken.count { "Password" in it })
    }

    @Test
    fun aRevealedSecretShowsItsValueAndOffersToHideIt() {
        rule.setKit { InsetGroup { row { SecretRow("Password", revealed = "hunter2", onReveal = {}, onCopy = {}) } } }
        rule.onNodeWithText("hunter2").assertExists()
        rule.onNodeWithContentDescription(text(R.string.hide, "Password")).assertExists()
    }

    @Test
    fun theCodeReadsInTwoHalvesWithItsTimeAndCopiesWhole() {
        val copied = mutableListOf<String>()
        rule.setKit {
            InsetGroup {
                row { CodeRow("One-time code", TotpNow("381492", 30u, 12u), failed = false, onCopy = { copied += it }) }
            }
        }
        rule.onNodeWithText("381 492", substring = true).assertExists()
        rule.onNodeWithContentDescription(
            RuntimeEnvironment.getApplication().resources.getQuantityString(R.plurals.item_seconds_remaining, 12, 12),
        ).assertExists()
        rule.onNodeWithContentDescription(text(R.string.copy, "One-time code")).performClick()
        assertEquals(listOf("381492"), copied)
    }

    @Test
    fun aFailedCodeSaysSoAndOffersNoCopy() {
        rule.setKit { InsetGroup { row { CodeRow("One-time code", now = null, failed = true, onCopy = {}) } } }
        rule.onNodeWithText(text(R.string.item_code_failed), substring = true).assertExists()
        rule.onAllNodesWithContentDescription(text(R.string.copy, "One-time code")).assertCountEquals(0)
    }

    @Test
    fun codesSplitInTheMiddle() {
        assertEquals("381 492", groupedCode("381492"))
        assertEquals("1234 5678", groupedCode("12345678"))
        assertEquals("1234", groupedCode("1234"))
    }
}
