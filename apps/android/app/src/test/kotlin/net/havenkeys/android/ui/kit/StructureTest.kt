package net.havenkeys.android.ui.kit

import androidx.compose.ui.semantics.Role
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.getUnclippedBoundsInRoot
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.isHeading
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class StructureTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aHeaderIsAHeadingAndItsActionAButton() {
        var cleared = 0
        rule.setKit { SectionHeader("Recent searches", action = SectionAction("Clear") { cleared++ }) }
        rule.onNodeWithText("Recent searches").assert(isHeading())
        rule.onNode(hasText("Clear") and hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, cleared)
    }

    @Test
    fun aGroupStacksItsRowsInOrder() {
        rule.setKit {
            InsetGroup {
                row { GroupRow { GroupRowText("One") } }
                row { GroupRow { GroupRowText("Two") } }
                row { GroupRow { GroupRowText("Three") } }
            }
        }
        val tops = listOf("One", "Two", "Three").map { rule.onNodeWithText(it).getUnclippedBoundsInRoot().top }
        assertEquals(tops.sorted(), tops)
    }

    @Test
    fun aTappableRowIsAButtonAndAPlainRowIsNot() {
        var opened = 0
        rule.setKit {
            InsetGroup {
                row {
                    GroupRow(onClick = { opened++ }, onClickLabel = "Open") {
                        GroupRowText("Auto-lock", "After 5 minutes")
                    }
                }
                row { GroupRow { GroupRowField("Username", "sam@example.com") } }
            }
        }
        rule.onNode(hasText("Auto-lock") and hasRole(Role.Button)).assert(hasText("After 5 minutes"))
            .assertHeightIsAtLeast(52.dp).performClick()
        assertEquals(1, opened)
        rule.onNodeWithText("sam@example.com").assertHasNoClickAction()
    }

    @Test
    fun anItemRowReadsAsOneButtonWithItsMarks() {
        var opened = 0
        rule.setKit {
            ItemRow(
                "GitHub",
                "sam@example.com",
                RowLeading.Monogram("GitHub"),
                onClick = { opened++ },
                hasPasskey = true,
                hasCode = true,
            )
        }
        rule.onNode(hasClickAction())
            .assert(hasRole(Role.Button))
            .assert(hasText("GitHub"))
            .assert(hasText("sam@example.com"))
            .assert(hasContentDescription("Passkey"))
            .assert(hasContentDescription("One-time code"))
            .assertHeightIsAtLeast(64.dp)
            .performClick()
        assertEquals(1, opened)
        // The monogram is decoration: TalkBack reads the title once.
        rule.onNodeWithText("G").assertDoesNotExist()
    }

    @Test
    fun noMarksNoMarkLabels() {
        rule.setKit { ItemRow("Wi-Fi", null, RowLeading.Glyph(HavenIcon.Note, soft = true), onClick = {}) }
        val row = rule.onNode(hasClickAction()).fetchSemanticsNode()
        assertFalse(hasContentDescription("Passkey").matches(row))
        assertFalse(hasContentDescription("One-time code").matches(row))
    }

    @Test
    fun aBlankTitleShowsTheKeyAndANamedOneShowsItsInitial() {
        rule.setKit {
            ItemTile(RowLeading.Monogram("   "))
            ItemTile(RowLeading.Monogram("GitHub"))
            ItemTile(RowLeading.Glyph(HavenIcon.Note))
        }
        // The tile hides its children from the merged tree, so look at the unmerged one.
        rule.onNodeWithTag(tileGlyphTag(HavenIcon.Key), useUnmergedTree = true).assertExists()
        rule.onNodeWithTag(tileGlyphTag(HavenIcon.Note), useUnmergedTree = true).assertExists()
        rule.onNodeWithText("G", useUnmergedTree = true).assertExists()
        assertEquals(
            2,
            rule.onAllNodes(
                hasTestTag(tileGlyphTag(HavenIcon.Key)) or hasTestTag(tileGlyphTag(HavenIcon.Note)),
                useUnmergedTree = true,
            ).fetchSemanticsNodes().size,
        )
    }

    @Test
    fun monogramsTakeTheFirstCharacterAsTheReaderSeesIt() {
        assertEquals("G", monogramOf("github"))
        assertEquals("É", monogramOf("  émile"))
        assertEquals("😀", monogramOf("😀 Fun"))
        assertEquals("東", monogramOf("東京"))
        assertEquals("ß", monogramOf("ßeta"))
        assertNull(monogramOf("   "))
        assertNull(monogramOf(""))
        assertTrue(monogramOf("😀 Fun")!!.length == 2)
    }
}
