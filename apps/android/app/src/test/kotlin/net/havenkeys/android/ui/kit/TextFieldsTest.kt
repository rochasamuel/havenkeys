package net.havenkeys.android.ui.kit

import android.text.InputType
import android.view.inputmethod.EditorInfo
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.ExperimentalComposeUiApi
import androidx.compose.ui.platform.InterceptPlatformTextInput
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
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
import kotlinx.coroutines.awaitCancellation
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

@RunWith(RobolectricTestRunner::class)
class TextFieldsTest {
    @get:Rule
    val rule = createComposeRule()

    private fun texts(node: androidx.compose.ui.test.SemanticsNodeInteraction): List<String> =
        node.fetchSemanticsNode().config.getOrNull(SemanticsProperties.Text).orEmpty().map { it.text }

    private fun descriptions(node: androidx.compose.ui.test.SemanticsNodeInteraction): List<String> =
        node.fetchSemanticsNode().config.getOrNull(SemanticsProperties.ContentDescription).orEmpty()

    @Test
    fun typingFillsTheStateAndTheLabelIsReadOnceWithTheText() {
        val state = TextFieldState()
        rule.setKit { HavenTextField(state, label = "Username") }
        val field = rule.onNode(hasSetTextAction())
        field.assertHeightIsAtLeast(48.dp)
        field.performTextInput("sam")
        rule.runOnIdle { assertEquals("sam", state.text.toString()) }
        // The label is part of the field's own merged node, not a description that hides the text.
        val merged = texts(field)
        assertEquals(1, merged.count { it == "Username" })
        assertEquals("sam", field.fetchSemanticsNode().config.getOrNull(SemanticsProperties.EditableText)?.text)
        assertTrue(descriptions(field).none { it == "Username" })
        // ... and not a second TalkBack stop.
        assertEquals(1, rule.onAllNodes(SemanticsMatcher("has text Username") {
            it.config.getOrNull(SemanticsProperties.Text)?.any { t -> t.text == "Username" } == true
        }).fetchSemanticsNodes().size)
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
    fun aSecretIsAPasswordFieldWhoseLabelMergesAndWhoseValueIsNeverADescription() {
        val state = TextFieldState()
        rule.setKit {
            var shown by remember { mutableStateOf(false) }
            SecretTextField(state, "Master password", revealed = shown, onRevealChange = { shown = it })
        }
        val field = rule.onNode(hasSetTextAction())
        field.performTextInput("hunter2")
        field.assert(SemanticsMatcher.keyIsDefined(SemanticsProperties.Password)).assertHeightIsAtLeast(48.dp)
        rule.runOnIdle { assertEquals("hunter2", state.text.toString()) }
        assertEquals(1, texts(field).count { it == "Master password" })
        assertTrue(descriptions(field).none { it == "Master password" || it.contains("hunter2") })
        assertTrue(texts(field).none { it.contains("hunter2") })
        rule.onAllNodes(SemanticsMatcher("has text Master password") {
            it.config.getOrNull(SemanticsProperties.Text)?.any { t -> t.text == "Master password" } == true
        }).assertCountEquals(1)
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

    /** What the keyboard is told when this field is focused, seen from outside the kit's own interceptor. */
    @OptIn(ExperimentalComposeUiApi::class)
    private fun editorInfoOf(content: @androidx.compose.runtime.Composable () -> Unit): EditorInfo {
        val info = EditorInfo()
        rule.setKit {
            InterceptPlatformTextInput(
                interceptor = { request, _ ->
                    request.createInputConnection(info)
                    awaitCancellation()
                },
                content = content,
            )
        }
        rule.onNode(hasSetTextAction()).performClick()
        rule.waitForIdle()
        return info
    }

    @Test
    fun aSecretFieldAsksForAPasswordKeyboardWithoutAutocorrectOrLearning() {
        val info = editorInfoOf {
            SecretTextField(TextFieldState(), "Master password", revealed = false, onRevealChange = {})
        }
        assertTrue(info.inputType and InputType.TYPE_TEXT_VARIATION_PASSWORD == InputType.TYPE_TEXT_VARIATION_PASSWORD)
        assertEquals(0, info.inputType and InputType.TYPE_TEXT_FLAG_AUTO_CORRECT)
        assertTrue(info.imeOptions and EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING != 0)
    }

    @Test
    fun anOrdinaryFieldAsksTheKeyboardNotToLearn() {
        val info = editorInfoOf { HavenTextField(TextFieldState(), label = "Title") }
        assertTrue(info.imeOptions and EditorInfo.IME_FLAG_NO_PERSONALIZED_LEARNING != 0)
    }

    @Test
    fun aHintIsReadWithTheFieldAndAnErrorTakesItsPlace() {
        var error by mutableStateOf<String?>(null)
        rule.setKit {
            SecretTextField(
                TextFieldState(),
                "Secret Key",
                revealed = false,
                onRevealChange = {},
                error = error,
                hint = "On your Emergency Kit",
            )
        }
        val field = rule.onNode(hasSetTextAction())
        assertTrue(texts(field).contains("On your Emergency Kit"))
        error = "Check the Secret Key"
        rule.waitForIdle()
        assertTrue(texts(field).none { it == "On your Emergency Kit" })
        field.assert(SemanticsMatcher.expectValue(SemanticsProperties.Error, "Check the Secret Key"))
    }

    @Test
    fun anOrdinaryFieldShowsItsHintToo() {
        rule.setKit { HavenTextField(TextFieldState(), "Server", hint = "The address your server answers on") }
        assertTrue(texts(rule.onNode(hasSetTextAction())).contains("The address your server answers on"))
    }
}
