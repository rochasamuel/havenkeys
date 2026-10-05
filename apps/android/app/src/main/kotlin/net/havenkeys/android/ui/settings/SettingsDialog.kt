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
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.KeyboardType
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField

enum class SettingsDialog { BIOMETRIC_PASSWORD, SIGN_OUT, REMOVE, DELETE_ACCOUNT }

@Composable
internal fun SettingsDialogs(
    dialog: SettingsDialog?,
    viewModel: SettingsViewModel,
    onClose: () -> Unit,
    onPassword: (String) -> Unit,
    onDeleteAccount: (email: String, password: String) -> Unit,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
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
                viewModel.clearAccountErrors()
                onClose()
            },
            onRemove = viewModel::removeDevice,
        )
        SettingsDialog.DELETE_ACCOUNT -> DeleteAccountDialog(
            state = state,
            onDismiss = {
                viewModel.clearAccountErrors()
                onClose()
            },
            onDelete = onDeleteAccount,
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

/**
 * Two steps: what deleting means (and that the backup is made on the
 * desktop), then the email and the master password. The password lives only
 * in this dialog's field, cleared after each attempt and when the dialog
 * leaves the composition. Rust checks both.
 */
@Composable
private fun DeleteAccountDialog(
    state: SettingsUiState,
    onDismiss: () -> Unit,
    onDelete: (email: String, password: String) -> Unit,
) {
    var confirming by rememberSaveable { mutableStateOf(false) }
    if (!confirming) {
        DeleteAccountExplain(onDismiss, onContinue = { confirming = true })
        return
    }
    val email = remember { TextFieldState() }
    val password = remember { TextFieldState() }
    DisposableEffect(password) { onDispose { password.clearText() } }
    var revealed by remember { mutableStateOf(false) }
    val prompt = state.email?.let { stringResource(R.string.settings_type_to_confirm, it) }
        ?: stringResource(R.string.settings_type_email)
    val error = state.deleteErrorCode?.let { stringResource(deleteErrorText(it)) }
    HavenDialog(
        title = stringResource(R.string.settings_delete_title),
        onDismiss = onDismiss,
        confirm = DialogAction(
            stringResource(R.string.settings_delete_confirm),
            {
                val typed = password.text.toString()
                password.clearText()
                onDelete(email.text.toString(), typed)
            },
            danger = true,
        ),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
        confirmEnabled = email.text.isNotBlank() && password.text.isNotEmpty(),
        busy = state.deleting,
        answerKey = state.deleteFailures,
        content = {
            InsetGroup {
                row {
                    HavenTextField(
                        email,
                        prompt,
                        enabled = !state.deleting,
                        keyboardOptions = KeyboardOptions(
                            keyboardType = KeyboardType.Email,
                            autoCorrectEnabled = false,
                        ),
                    )
                }
                row {
                    SecretTextField(
                        password,
                        stringResource(R.string.settings_delete_password),
                        revealed = revealed,
                        onRevealChange = { revealed = it },
                        error = error,
                        enabled = !state.deleting,
                    )
                }
            }
        },
    )
}

/** Step one of deleting: what it means, and that the backup is made on the desktop. */
@Composable
private fun DeleteAccountExplain(onDismiss: () -> Unit, onContinue: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.settings_delete_title),
        onDismiss = onDismiss,
        confirm = DialogAction(stringResource(R.string.settings_delete_anyway), onContinue, danger = true),
        message = stringResource(R.string.settings_delete_explain),
        dismiss = DialogAction(stringResource(R.string.settings_cancel), onDismiss),
    )
}

@StringRes
private fun deleteErrorText(code: String): Int = when (code) {
    INVALID_INPUT -> R.string.settings_remove_mismatch
    "internal" -> R.string.settings_delete_failed
    else -> errorText(code)
}

@StringRes
private fun removeErrorText(code: String): Int = when (code) {
    INVALID_INPUT -> R.string.settings_remove_mismatch
    "internal" -> R.string.settings_remove_failed
    else -> errorText(code)
}
