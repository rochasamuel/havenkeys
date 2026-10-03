package net.havenkeys.android.ui.kit

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.text.BasicSecureTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.KeyboardActionHandler
import androidx.compose.foundation.text.input.TextFieldDecorator
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.TextObfuscationMode
import androidx.compose.foundation.text.input.rememberTextFieldState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.error
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.tooling.preview.PreviewLightDark
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * A secret field: mono, fully masked (no last-character flash) until the
 * user reveals it with the eye. The caller owns [revealed], so its lock
 * wipe resets it. Password semantics, password keyboard, no autocorrect,
 * no learning, no cut or copy; TalkBack reads the label (merged into the field's node) and
 * names the eye "Show"/"Hide" and the label; the value is never a description.
 */
@Composable
fun SecretTextField(
    state: TextFieldState,
    label: String,
    revealed: Boolean,
    onRevealChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    error: String? = null,
    enabled: Boolean = true,
    imeAction: ImeAction = ImeAction.Done,
    onKeyboardAction: KeyboardActionHandler? = null,
) {
    val interaction = remember { MutableInteractionSource() }
    val focused by interaction.collectIsFocusedAsState()
    val colors = HavenTheme.colors
    val eyeLabel = stringResource(if (revealed) R.string.hide else R.string.reveal, label)
    KitTextInput {
        BasicSecureTextField(
            state = state,
            modifier = modifier.fillMaxWidth().semantics { if (error != null) error(error) },
            enabled = enabled,
            textStyle = HavenTheme.type.secret.copy(color = colors.textStrong),
            keyboardOptions = KeyboardOptions(
                keyboardType = KeyboardType.Password,
                autoCorrectEnabled = false,
                imeAction = imeAction,
            ),
            onKeyboardAction = onKeyboardAction,
            interactionSource = interaction,
            cursorBrush = SolidColor(colors.brass),
            textObfuscationMode = if (revealed) TextObfuscationMode.Visible else TextObfuscationMode.Hidden,
            decorator = TextFieldDecorator { field ->
                FieldRow(
                    label = label,
                    error = error,
                    focused = focused,
                    trailing = {
                        HavenIconButton(
                            if (revealed) HavenIcon.EyeOff else HavenIcon.Eye,
                            eyeLabel,
                            onClick = { onRevealChange(!revealed) },
                        )
                    },
                    field = field,
                )
            },
        )
    }
}

@PreviewLightDark
@Composable
private fun SecretTextFieldPreview() {
    KitPreview {
        InsetGroup {
            row {
                SecretTextField(
                    rememberTextFieldState("hunter2hunter2"),
                    "Password",
                    revealed = false,
                    onRevealChange = {},
                )
            }
            row {
                SecretTextField(
                    rememberTextFieldState("hunter2hunter2"),
                    "Password",
                    revealed = true,
                    onRevealChange = {},
                )
            }
        }
    }
}
