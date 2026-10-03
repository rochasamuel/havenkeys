package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.assertTouchTarget
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class AddSheetTest {
    @get:Rule
    val rule = createComposeRule()

    private var picked: AddTile? = null

    private fun show(online: Boolean) = rule.setKit {
        AddSheet(tilesFor(online), onPick = { picked = it }, onDismiss = {})
    }

    private fun tile(label: String) = rule.onNode(hasText(label) and hasRole(Role.Button))

    @Test
    fun offlineTheItemTilesAreDimmedAndSayWhy() {
        show(online = false)
        rule.onNodeWithText("New item").assert(isHeading())
        listOf("Login", "Secure note", "Card").forEach { tile(it).assertIsNotEnabled() }
        rule.onNodeWithText("Adding needs a connection").assertIsDisplayed()
        tile("Generate password").assertIsEnabled().assertTouchTarget().performClick()
        assertEquals(AddTile.GENERATOR, picked)
    }

    @Test
    fun onlineEveryTileOpensItsForm() {
        show(online = true)
        rule.onNodeWithText("Adding needs a connection").assertDoesNotExist()
        tile("Card").assertTouchTarget().performClick()
        assertEquals(AddTile.CARD, picked)
    }
}
