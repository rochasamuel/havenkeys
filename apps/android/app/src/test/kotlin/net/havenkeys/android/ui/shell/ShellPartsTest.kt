package net.havenkeys.android.ui.shell

import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.RowLeading
import net.havenkeys.android.ui.kit.hasRole
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.DarkHavenColors
import net.havenkeys.android.ui.theme.LightHavenColors
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

@RunWith(RobolectricTestRunner::class)
class ShellPartsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun summary(
        kind: ItemKind,
        subtitle: String? = null,
        website: String? = null,
        totp: Boolean = false,
        passkey: Boolean = false,
    ) = ItemSummary("id-1", kind, "GitHub", subtitle, website, totp, passkey, 0, 0)

    @Test
    fun eachKindHasItsTile() {
        assertEquals(RowLeading.Monogram("GitHub"), summary(ItemKind.LOGIN).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.Note, soft = true), summary(ItemKind.SECURE_NOTE).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.Card), summary(ItemKind.CARD).leading())
        assertEquals(RowLeading.Glyph(HavenIcon.IdCard), summary(ItemKind.IDENTITY).leading())
    }

    @Test
    fun theSecondLineIsTheSubtitleElseTheWebsite() {
        assertEquals("sam", summary(ItemKind.LOGIN, subtitle = "sam", website = "github.com").secondLine())
        assertEquals("github.com", summary(ItemKind.LOGIN, website = "github.com").secondLine())
        assertNull(summary(ItemKind.SECURE_NOTE).secondLine())
    }

    @Test
    fun aSummaryRowOpensWithItsOrigin() {
        var opened: Pair<String, String>? = null
        rule.setKit {
            SummaryRow(
                summary(ItemKind.LOGIN, subtitle = "sam@example.com", totp = true, passkey = true),
                Origins.RECENT,
                onOpen = { id, origin -> opened = id to origin },
            )
        }
        rule.onNode(hasClickAction()).assert(hasText("GitHub")).assert(hasText("sam@example.com")).performClick()
        assertEquals("id-1" to Origins.RECENT, opened)
    }

    @Test
    fun slicesKnowTheirPlace() {
        assertEquals(SlicePosition.Single, slicePosition(0, 1))
        assertEquals(SlicePosition.First, slicePosition(0, 3))
        assertEquals(SlicePosition.Middle, slicePosition(1, 3))
        assertEquals(SlicePosition.Last, slicePosition(2, 3))
    }

    @Test
    fun aLazyGroupShowsEveryRowAsAButton() {
        rule.setKit {
            LazyColumn {
                insetGroup(listOf("One", "Two", "Three"), key = { it }) { GroupRow(onClick = {}) { GroupRowText(it) } }
            }
        }
        listOf("One", "Two", "Three").forEach { rule.onNode(hasText(it) and hasRole(Role.Button)).assertIsDisplayed() }
    }

    @Test
    fun aSettlingRowTakesATapBeforeItHasSettled() {
        var taps = 0
        rule.mainClock.autoAdvance = false
        rule.setKit { Settle(index = 3) { HavenButton("Login", onClick = { taps++ }) } }
        rule.mainClock.advanceTimeByFrame()
        rule.onNodeWithText("Login").performClick()
        assertEquals(1, taps)
    }

    @Test
    fun theSearchPillStandsOffThePaneInBothThemes() {
        // The light field is white like the pane, so the pill takes the hover green there.
        assertNotEquals(LightHavenColors.pane, LightHavenColors.searchPill)
        assertEquals(LightHavenColors.hover, LightHavenColors.searchPill)
        assertEquals(DarkHavenColors.field, DarkHavenColors.searchPill)
    }
}
