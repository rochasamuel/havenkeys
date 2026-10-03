package net.havenkeys.android.ui.kit

import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertContentDescriptionEquals
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertHeightIsAtLeast
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class TextFieldsTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun typingFillsTheStateAndTheLabelNamesTheFieldOnce() {
        val state = TextFieldState()
        rule.setKit { HavenTextField(state, label = "Username") }
        val field = rule.onNode(hasSetTextAction())
        field.assertContentDescriptionEquals("Username").assertHeightIsAtLeast(48.dp)
        field.performTextInput("sam")
        rule.runOnIdle { assertEquals("sam", state.text.toString()) }
        // The visible label is not a second TalkBack stop.
        rule.onAllNodesWithText("Username").assertCountEquals(0)
    }

    @Test
    fun anErrorIsAnnouncedAsTheFieldsError() {
        rule.setKit { HavenTextField(TextFieldState("http://vault"), label = "Server", error = "Use https://") }
        rule.onNode(hasSetTextAction()).assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Use https://"))
    }

    @Test
    fun anOrdinaryFieldIsNotAPasswordField() {
        rule.setKit { HavenTextField(TextFieldState(), label = "Title") }
        rule.onNode(hasSetTextAction()).assert(SemanticsMatcher.keyNotDefined(SemanticsProperties.Password))
    }

    @Test
    fun aSecretIsAPasswordFieldNamedByItsLabel() {
        val state = TextFieldState()
        rule.setKit {
            var shown by remember { mutableStateOf(false) }
            SecretTextField(state, "Master password", revealed = shown, onRevealChange = { shown = it })
        }
        val field = rule.onNode(hasSetTextAction())
        field.performTextInput("hunter2")
        field.assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password))
            .assertContentDescriptionEquals("Master password")
            .assertHeightIsAtLeast(48.dp)
        rule.runOnIdle { assertEquals("hunter2", state.text.toString()) }
    }

    @Test
    fun theEyeShowsAndHidesAndSaysWhich() {
        var revealed = false
        rule.setKit {
            var shown by remember { mutableStateOf(false) }
            SecretTextField(
                TextFieldState("hunter2"),
                "Master password",
                revealed = shown,
                onRevealChange = { shown = it; revealed = it },
            )
        }
        rule.onNodeWithContentDescription("Show Master password")
            .assert(hasRole(Role.Button)).assertTouchTarget().performClick()
        assertEquals(true, revealed)
        rule.onNodeWithContentDescription("Hide Master password").assertExists()
        rule.onNodeWithContentDescription("Show Master password").assertDoesNotExist()
    }
}
