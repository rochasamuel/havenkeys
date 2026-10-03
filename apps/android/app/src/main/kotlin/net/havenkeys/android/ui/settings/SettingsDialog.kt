package net.havenkeys.android.ui.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.clearText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField

enum class SettingsDialog { BIOMETRIC_PASSWORD, SIGN_OUT, REMOVE }

@Composable
internal fun SettingsDialogs(
    dialog: SettingsDialog?,
    state: SettingsUiState,
    viewModel: SettingsViewModel,
    onClose: () -> Unit,
    onPassword: (String) -> Unit,
) {
    when (dialog) {
        SettingsDialog.BIOMETRIC_PASSWORD -> PasswordDialog(
            onDismiss = onClose,
            onSubmit = { password ->
                onClose()
                onPassword(password)
            },
        )
        SettingsDialog.SIGN_OUT -> HavenDialog(
            title = stringResource(R.string.settings_sign_out_confirm),
            onDismiss = onClose,
            confirm = DialogAction(stringResource(R.string.settings_sign_out), {
                onClose()
                viewModel.signOut()
            }),
            dismiss = DialogAction(stringResource(R.string.settings_cancel), onClose),
        )
        SettingsDialog.REMOVE -> RemoveDialog(
            state = state,
            onDismiss = {
                viewModel.clearRemoveError()
                onClose()
            },
            onRemove = viewModel::removeDevice,
        )
        null -> Unit
    }
}

/**
 * The master password lives only in this dialog's `remember`ed field (never
 * saved) until it is handed to Rust. The field is cleared as it goes and
 * whenever the dialog leaves the composition (closed, or the screen is gone
 * after a lock).
 */
@Composable
private fun PasswordDialog(onDismiss: () -> Unit, onSubmit: (String) -> Unit) {
    val field = remember { TextFieldState() }
    DisposableEffect(field) { onDispose { field.clearText() } }
    var revealed by remember { mutableStateOf(false) }
    val submit = {
        val typed = field.text.toString()
        if (typed.isNotEmpty()) {
            field.clearText()
            onSubmit(typed)
        }
    }
    HavenDialog(
        title = stringResource(R.string.settings_biometric),
        onDismiss = onDismiss,
        confirm = DialogAction(stringResource(R.string.settings_turn_on), submit),
        message = stringResource(R.string.settings_biometric_password),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
        confirmEnabled = field.text.isNotEmpty(),
        content = {
            InsetGroup {
                row {
                    SecretTextField(
                        field,
                        stringResource(R.string.unlock_password_hint),
                        revealed = revealed,
                        onRevealChange = { revealed = it },
                        onKeyboardAction = { submit() },
                    )
                }
            }
        },
    )
}

/** Rust compares the typed email with the account's; this dialog only passes it on and shows the answer. */
@Composable
private fun RemoveDialog(state: SettingsUiState, onDismiss: () -> Unit, onRemove: (String) -> Unit) {
    val field = remember { TextFieldState() }
    val prompt = state.email?.let { stringResource(R.string.settings_type_to_confirm, it) }
        ?: stringResource(R.string.settings_type_email)
    val error = state.removeErrorCode?.let { stringResource(removeErrorText(it)) }
    HavenDialog(
        title = stringResource(R.string.settings_remove_title),
        onDismiss = onDismiss,
        confirm = DialogAction(
            stringResource(R.string.settings_remove_title),
            { onRemove(field.text.toString()) },
            danger = true,
        ),
        message = stringResource(R.string.settings_remove_note),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
        confirmEnabled = field.text.isNotBlank(),
        busy = state.removing,
        // A failure counter, not the message: the same error twice must still re-arm the button.
        answerKey = state.removeFailures,
        content = {
            InsetGroup {
                row {
                    HavenTextField(
                        field,
                        prompt,
                        error = error,
                        enabled = !state.removing,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Email,
                            autoCorrectEnabled = false,
                        ),
                    )
                }
            }
        },
    )
}

@StringRes
private fun removeErrorText(code: String): Int = when (code) {
    INVALID_INPUT -> R.string.settings_remove_mismatch
    "internal" -> R.string.settings_remove_failed
    else -> errorText(code)
}
