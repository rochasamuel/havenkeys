package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.Column
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.assertAll
import androidx.compose.ui.test.onChildren
import androidx.compose.ui.test.onNodeWithTag
import org.junit.Assert.assertTrue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertHasNoClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class ActionsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aButtonIsAButtonNamedByItsLabel() {
        var clicks = 0
        rule.setKit { HavenButton("Save", onClick = { clicks++ }) }
        rule.onNode(hasText("Save") and hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }

    @Test
    fun aDisabledButtonSaysSoAndDoesNothing() {
        var clicks = 0
        rule.setKit { HavenButton("Save", onClick = { clicks++ }, enabled = false) }
        rule.onNodeWithText("Save").assertIsNotEnabled().performClick()
        assertEquals(0, clicks)
    }

    @Test
    fun everyStyleKeepsTheTouchTarget() {
        rule.setKit { Column { ButtonStyle.entries.forEach { HavenButton(it.name, onClick = {}, style = it) } } }
        ButtonStyle.entries.forEach { rule.onNodeWithText(it.name).assert(hasRole(Role.Button)).assertTouchTarget() }
    }

    @Test
    fun anIconButtonIsNamedAndBigEnough() {
        var clicks = 0
        rule.setKit { HavenIconButton(HavenIcon.Lock, "Lock", onClick = { clicks++ }) }
        rule.onNodeWithContentDescription("Lock").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }

    @Test
    fun copyNamesTheFieldThenSaysCopiedForAMoment() {
        rule.mainClock.autoAdvance = false
        var copies = 0
        rule.setKit { CopyButton("Password", onCopy = { copies++ }) }
        val button = rule.onNodeWithContentDescription("Copy Password")
        button.assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        rule.mainClock.advanceTimeBy(100)
        assertEquals(1, copies)
        button.assert(SemanticsMatcher.expectValue(SemanticsProperties.StateDescription, "Copied"))
        rule.mainClock.advanceTimeBy(COPIED_MILLIS + 500)
        button.assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.StateDescription))
    }

    @Test
    fun aPillIsAMarkerNotAControl() {
        rule.setKit { Pill("This device") }
        rule.onNodeWithText("This device").assertIsDisplayed().assertHasNoClickAction()
    }

    @Test
    fun aPillsGlyphWidensItButIsNotRead() {
        rule.setKit {
            Column {
                Pill("work", Modifier.testTag("plain"), tone = PillTone.Outline)
                Pill("work", Modifier.testTag("tagged"), tone = PillTone.Outline, icon = HavenIcon.Tag)
            }
        }
        val plain = rule.onNodeWithTag("plain").fetchSemanticsNode().size.width
        val tagged = rule.onNodeWithTag("tagged", useUnmergedTree = true).fetchSemanticsNode().size.width
        assertTrue(tagged > plain)
        // The glyph is decoration: the tagged pill reads as its text alone.
        rule.onNodeWithTag("tagged", useUnmergedTree = true).onChildren()
            .assertAll(SemanticsMatcher.keyNotDefined(SemanticsProperties.ContentDescription))
    }

    @Test
    fun theAddButtonIsNamed() {
        var clicks = 0
        rule.setKit { AddButton(onClick = { clicks++ }) }
        rule.onNodeWithContentDescription("New item").assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(1, clicks)
    }
}
