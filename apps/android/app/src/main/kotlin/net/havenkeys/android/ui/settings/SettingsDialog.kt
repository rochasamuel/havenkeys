package net.havenkeys.android.ui.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.components.errorText

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
        SettingsDialog.SIGN_OUT -> ConfirmDialog(
            text = stringResource(R.string.settings_sign_out_confirm),
            confirm = stringResource(R.string.settings_sign_out),
            onDismiss = onClose,
            onConfirm = {
                onClose()
                viewModel.signOut()
            },
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

/** The master password lives only here (`remember`, not saveable) until it is handed to Rust. */
@Composable
private fun PasswordDialog(onDismiss: () -> Unit, onSubmit: (String) -> Unit) {
    var password by remember { mutableStateOf("") }
    val submit = {
        if (password.isNotEmpty()) {
            val typed = password
            password = ""
            onSubmit(typed)
        }
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.settings_biometric)) },
        text = {
            Column {
                SecureDialogWindow()
                Text(stringResource(R.string.settings_biometric_password), modifier = Modifier.padding(bottom = 12.dp))
                OutlinedTextField(
                    value = password,
                    onValueChange = { password = it },
                    label = { Text(stringResource(R.string.unlock_password_hint)) },
                    singleLine = true,
                    visualTransformation = PasswordVisualTransformation(),
                    keyboardOptions = KeyboardOptions(
                        keyboardType = KeyboardType.Password,
                        autoCorrectEnabled = false,
                        imeAction = ImeAction.Done,
                    ),
                    keyboardActions = KeyboardActions(onDone = { submit() }),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = {
            TextButton(onClick = submit, enabled = password.isNotEmpty()) {
                Text(stringResource(R.string.settings_turn_on))
            }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.settings_cancel)) } },
    )
}

@Composable
private fun ConfirmDialog(text: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit) {
    AlertDialog(
        onDismissRequest = onDismiss,
        text = { Text(text) },
        confirmButton = { TextButton(onClick = onConfirm) { Text(confirm) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(stringResource(R.string.settings_cancel)) } },
    )
}

/** Rust compares the typed email with the account's; this dialog only passes it on. */
@Composable
private fun RemoveDialog(state: SettingsUiState, onDismiss: () -> Unit, onRemove: (String) -> Unit) {
    var typed by remember { mutableStateOf("") }
    val prompt = state.email?.let { stringResource(R.string.settings_type_to_confirm, it) }
        ?: stringResource(R.string.settings_type_email)
    AlertDialog(
        onDismissRequest = { if (!state.removing) onDismiss() },
        title = { Text(stringResource(R.string.settings_remove_title)) },
        text = {
            Column {
                SecureDialogWindow()
                Text(stringResource(R.string.settings_remove_note), modifier = Modifier.padding(bottom = 12.dp))
                OutlinedTextField(
                    value = typed,
                    onValueChange = { typed = it },
                    label = { Text(prompt) },
                    singleLine = true,
                    enabled = !state.removing,
                    isError = state.removeErrorCode != null,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Email, autoCorrectEnabled = false),
                    modifier = Modifier.fillMaxWidth(),
                )
                state.removeErrorCode?.let { code ->
                    Text(
                        stringResource(removeErrorText(code)),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodyMedium,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { onRemove(typed) }, enabled = typed.isNotBlank() && !state.removing) {
                Text(stringResource(R.string.settings_remove_title), color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !state.removing) {
                Text(stringResource(R.string.settings_cancel))
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

/** A setting with a few fixed values, chosen in a dialog. */
@Composable
internal fun ChoiceRow(
    @StringRes label: Int,
    choices: List<UInt>,
    selected: UInt,
    text: @Composable (UInt) -> String,
    onSelect: (UInt) -> Unit,
) {
    var open by remember { mutableStateOf(false) }
    ListItem(
        headlineContent = { Text(stringResource(label)) },
        supportingContent = { Text(text(selected)) },
        colors = rowColors(),
        modifier = Modifier.clickable { open = true },
    )
    if (open) {
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text(stringResource(label)) },
            text = {
                Column {
                    choices.forEach { choice ->
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            modifier = Modifier
                                .fillMaxWidth()
                                .selectable(selected = choice == selected, role = Role.RadioButton) {
                                    open = false
                                    if (choice != selected) onSelect(choice)
                                }
                                .padding(vertical = 4.dp),
                        ) {
                            RadioButton(selected = choice == selected, onClick = null)
                            Text(text(choice), modifier = Modifier.padding(start = 12.dp))
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { open = false }) { Text(stringResource(R.string.settings_cancel)) }
            },
        )
    }
}
