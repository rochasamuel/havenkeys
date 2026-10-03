package net.havenkeys.android.ui.components

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.DarkHavenColors
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class ScreenPartsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun text(id: Int) = RuntimeEnvironment.getApplication().getString(id)

    @Test
    fun theBarHasBackLockAndItsActionsAndSaysOfflineOnlyWhenOffline() {
        var online by mutableStateOf(true)
        val taps = mutableListOf<String>()
        rule.setKit {
            ScreenBar(onBack = { taps += "back" }, online = online, onLock = { taps += "lock" }) {
                HavenButton("Save", onClick = { taps += "save" })
            }
        }
        rule.onNodeWithContentDescription(text(R.string.item_back)).assert(hasRole(Role.Button)).assertTouchTarget()
            .performClick()
        rule.onNodeWithContentDescription(text(R.string.vault_lock_now)).assertTouchTarget().performClick()
        rule.onNodeWithText("Save").performClick()
        assertEquals(listOf("back", "lock", "save"), taps)
        rule.onAllNodesWithText(text(R.string.vault_offline)).assertCountEquals(0)
        online = false
        rule.onNodeWithText(text(R.string.vault_offline)).assertExists()
    }

    @Test
    fun theOfflineMarkerIsAnnouncedPolitely() {
        rule.setKit { ScreenBar(onBack = {}, online = false, onLock = {}) }
        val polite = SemanticsMatcher("polite live region") {
            it.config.getOrNull(SemanticsProperties.LiveRegion) == LiveRegionMode.Polite
        }
        rule.onNode(polite, useUnmergedTree = true).assertExists()
        rule.onNodeWithText(text(R.string.vault_offline)).assertExists()
    }

    @Test
    fun aMaskedValueSaysHiddenAndNeverItsLength() {
        rule.setKit { MaskedValue("Password") }
        rule.onNodeWithContentDescription(
            RuntimeEnvironment.getApplication().getString(R.string.hidden, "Password"),
        ).assertExists()
        rule.onAllNodesWithText(MASK).assertCountEquals(0)
    }

    @Test
    fun digitsAndSymbolsTakeTheirOwnColours() {
        val shown = colourised("a1!", DarkHavenColors)
        assertEquals("a1!", shown.text)
        val colours = shown.spanStyles.associate { it.start to it.item.color }
        assertEquals(DarkHavenColors.digit, colours[1])
        assertEquals(DarkHavenColors.symbol, colours[2])
    }
}
