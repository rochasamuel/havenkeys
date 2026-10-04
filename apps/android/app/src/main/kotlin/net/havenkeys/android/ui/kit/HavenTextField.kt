package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.InputTransformation
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A text field the desktop way: the row is the field (no box, no border of
 * its own). A muted label above the value; on focus the row takes the
 * selection wash and an inset brass ring. Put it in an [InsetGroup] row.
 * TalkBack reads the label and the typed text as one node; an error is
 * the field's error. The keyboard is asked not to learn what is typed.
 * [hint] is a muted line under the value, read with the field; an error takes its place.
 */
@Composable
fun HavenTextField(
    state: TextFieldState,
    label: String,
    modifier: Modifier = Modifier,
    placeholder: String? = null,
    error: String? = null,
    enabled: Boolean = true,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    onKeyboardAction: KeyboardActionHandler? = null,
    lineLimits: TextFieldLineLimits = TextFieldLineLimits.SingleLine,
    inputTransformation: InputTransformation? = null,
    hint: String? = null,
) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val colors = HavenTheme.colors
    KitTextInput {
        BasicTextField(
            state = state,
            modifier = modifier.fillMaxWidth().semantics { if (error != null) error(error) },
            enabled = enabled,
            inputTransformation = inputTransformation,
            textStyle = HavenTheme.type.value.copy(color = colors.textStrong),
            keyboardOptions = keyboardOptions,
            onKeyboardAction = onKeyboardAction,
            lineLimits = lineLimits,
            interactionSource = interaction,
            cursorBrush = SolidColor(colors.brass),
            decorator = TextFieldDecorator { field ->
                FieldRow(label, error, focused, hint) {
                    Box {
                        if (placeholder != null && state.text.isEmpty()) {
                            HavenText(
                                placeholder,
                                Modifier.clearAndSetSemantics {},
                                style = HavenTheme.type.value,
                                color = colors.muted,
                            )
                        }
                        field()
                    }
                }
            },
        )
    }
}

/** The row every kit field draws: label, value, error, optional trailing control. */
@Composable
internal fun FieldRow(
    label: String,
    error: String?,
    focused: Boolean,
    hint: String? = null,
    trailing: (@Composable () -> Unit)? = null,
    field: @Composable () -> Unit,
) {
    val colors = HavenTheme.colors
    // The row's own outline, so the ring keeps the group's rounded corners instead of being cut by them.
    val shape = LocalRowShape.current
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .background(if (focused) colors.sel else Color.Transparent, shape)
            .then(if (focused) Modifier.border(1.5.dp, colors.brass, shape) else Modifier)
            .padding(
                start = HavenSpacing.rowX,
                end = if (trailing != null) 4.dp else HavenSpacing.rowX,
                top = 8.dp,
                bottom = 8.dp,
            ),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            // Not cleared: drawn inside the field's decorator, the label merges into the field's own
            // node, so TalkBack reads it with the typed text (a contentDescription would replace the text).
            HavenText(
                label,
                style = HavenTheme.type.label,
                color = if (error != null) colors.danger else colors.muted,
            )
            field()
            if (error != null) {
                HavenText(error, Modifier.clearAndSetSemantics {}, style = HavenTheme.type.label, color = colors.danger)
            } else if (hint != null) {
                // Not cleared: it merges into the field's node, so TalkBack reads it after the text.
                HavenText(hint, style = HavenTheme.type.label, color = colors.muted)
            }
        }
        trailing?.invoke()
    }
}

/** Brass selection handles, and no learning by the keyboard. */
@Composable
internal fun KitTextInput(content: @Composable () -> Unit) {
    val colors = HavenTheme.colors
    val selection = TextSelectionColors(handleColor = colors.brass, backgroundColor = colors.brass.copy(alpha = 0.32f))
    CompositionLocalProvider(LocalTextSelectionColors provides selection) {
        NoPersonalizedLearning(content)
    }
}

@PreviewLightDark
@Composable
private fun HavenTextFieldPreview() {
    KitPreview {
        InsetGroup {
            row { HavenTextField(rememberTextFieldState("sam@example.com"), "Username") }
            row { HavenTextField(rememberTextFieldState(), "Website", placeholder = "example.com") }
            row {
                HavenTextField(
                    rememberTextFieldState("http://vault"),
                    "Server",
                    error = "The address must start with https://",
                )
            }
        }
    }
}
