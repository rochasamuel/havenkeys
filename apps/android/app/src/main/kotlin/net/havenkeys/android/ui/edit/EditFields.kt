package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Casino
import androidx.compose.material.icons.outlined.Close
import androidx.compose.material.icons.outlined.Visibility
import androidx.compose.material.icons.outlined.VisibilityOff
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.MASK
import net.havenkeys.android.ui.item.fieldLabel
import net.havenkeys.android.ui.theme.HavenType
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.MatchKind

/** Long text the user edits: multi-line, and not masked once opened. */
private val longText = setOf("notes", "content", "card.notes", "identity.notes")

private val matchLabels = listOf(
    MatchKind.DOMAIN to R.string.edit_match_domain,
    MatchKind.ORIGIN to R.string.edit_match_origin,
    MatchKind.EXACT to R.string.edit_match_exact,
)

@Composable
internal fun EditFields(editor: EditorState, edit: ItemEdit, values: FieldValues) {
    Column(
        modifier = Modifier.padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // The identity's title is its name, built by Rust.
        if (edit.kind != ItemKind.IDENTITY) {
            OutlinedTextField(
                value = editor.title,
                onValueChange = { editor.title = it },
                label = { Text(stringResource(R.string.edit_title)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
        }
        if (edit.kind == ItemKind.LOGIN) Websites(editor)
        edit.fields.forEach { field ->
            key(field.key) { FieldEditor(field, editor, values) }
        }
        if (edit.hasCustomFields) {
            Text(
                text = stringResource(R.string.edit_custom_kept),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun Websites(editor: EditorState) {
    Column {
        Text(
            text = stringResource(R.string.edit_websites),
            style = MaterialTheme.typography.labelLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        editor.websites.forEach { row ->
            key(row) { WebsiteEditor(row, onRemove = { editor.websites.remove(row) }) }
        }
        TextButton(onClick = editor::addWebsite) { Text(stringResource(R.string.edit_add_website)) }
    }
}

@Composable
private fun WebsiteEditor(row: WebsiteRow, onRemove: () -> Unit) {
    var choosing by remember { mutableStateOf(false) }
    Column(Modifier.padding(top = 8.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                value = row.url,
                onValueChange = { row.url = it },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false),
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = onRemove) {
                Icon(Icons.Outlined.Close, contentDescription = stringResource(R.string.edit_remove_website))
            }
        }
        Box {
            TextButton(onClick = { choosing = true }) {
                Text(stringResource(matchLabels.first { it.first == row.match }.second))
            }
            DropdownMenu(expanded = choosing, onDismissRequest = { choosing = false }) {
                matchLabels.forEach { (match, label) ->
                    DropdownMenuItem(
                        text = { Text(stringResource(label)) },
                        onClick = {
                            row.match = match
                            choosing = false
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun FieldEditor(field: EditField, editor: EditorState, values: FieldValues) {
    val label = stringResource(fieldLabel(field.key))
    // Removal first: the old value is never shown for a field being removed.
    when {
        editor.isRemoved(field.key) -> RemovedRow(label, onUndo = { editor.undo(field.key) })
        field.kind == FieldKind.TEXT -> TextEditor(field.key, label, editor, values)
        field.kind == FieldKind.SECRET && field.present && !editor.isOpen(field.key) -> HiddenRow(
            label = label,
            masked = true,
            onChange = { loadSecret(field.key, editor, values) },
            onRemove = { editor.remove(field.key) },
        )
        field.kind == FieldKind.SECRET -> SecretEditor(field.key, label, editor, values)
        // A one-time code's key is never shown: it can be replaced or removed.
        field.present && !editor.isOpen(field.key) -> HiddenRow(
            label = label,
            masked = false,
            onChange = { editor.open(field.key) },
            onRemove = { editor.remove(field.key) },
        )
        else -> OutlinedTextField(
            value = editor.shown(field.key),
            onValueChange = { editor.type(field.key, it) },
            label = { Text(label) },
            placeholder = { Text(stringResource(R.string.edit_totp_placeholder)) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
            modifier = Modifier.fillMaxWidth(),
        )
    }
}

/** A hidden secret is read from Rust only when the user chooses to change it. */
private suspend fun loadSecret(key: String, editor: EditorState, values: FieldValues) {
    (values.viewModel.reveal(key) as? Outcome.Ok)?.let { editor.load(key, it.value) }
    // On failure it still opens, empty: typing replaces, leaving it empty keeps.
    editor.open(key)
}

@Composable
private fun TextEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val multiLine = key in longText
    val loading = key in values.loading
    OutlinedTextField(
        value = editor.shown(key),
        onValueChange = { editor.type(key, it) },
        label = { Text(label) },
        enabled = !loading,
        placeholder = if (key == "card.expiry") {
            { Text(stringResource(R.string.edit_expiry_placeholder)) }
        } else {
            null
        },
        singleLine = !multiLine,
        minLines = if (multiLine) 3 else 1,
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Text,
            autoCorrectEnabled = !(key == "card.holder" || key.startsWith("identity.") || key == "username"),
        ),
        modifier = Modifier.fillMaxWidth(),
    )
}

@Composable
private fun SecretEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val scope = rememberCoroutineScope()
    var visible by remember { mutableStateOf(false) }
    val multiLine = key in longText
    OutlinedTextField(
        value = editor.shown(key),
        onValueChange = { editor.type(key, it) },
        label = { Text(label) },
        singleLine = !multiLine,
        minLines = if (multiLine) 3 else 1,
        textStyle = if (multiLine) MaterialTheme.typography.bodyLarge else HavenType.secret,
        visualTransformation = if (multiLine || visible) VisualTransformation.None else PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(
            keyboardType = if (multiLine) KeyboardType.Text else KeyboardType.Password,
            autoCorrectEnabled = false,
        ),
        trailingIcon = if (multiLine) {
            null
        } else {
            {
                Row {
                    if (key == "password") {
                        IconButton(onClick = {
                            scope.launch {
                                val generated = values.viewModel.generate()
                                if (generated is Outcome.Ok) {
                                    editor.type(key, generated.value)
                                    visible = true
                                }
                            }
                        }) {
                            Icon(Icons.Outlined.Casino, contentDescription = stringResource(R.string.edit_generate))
                        }
                    }
                    IconButton(onClick = { visible = !visible }) {
                        if (visible) {
                            Icon(Icons.Outlined.VisibilityOff, stringResource(R.string.hide, label))
                        } else {
                            Icon(Icons.Outlined.Visibility, stringResource(R.string.reveal, label))
                        }
                    }
                }
            }
        },
        modifier = Modifier.fillMaxWidth(),
    )
}

/** A present value that is not shown: the fixed mask (never its length) or "Set up.", with Change and Remove. */
@Composable
private fun HiddenRow(label: String, masked: Boolean, onChange: suspend () -> Unit, onRemove: () -> Unit) {
    val scope = rememberCoroutineScope()
    FieldRow(label, if (masked) MASK else stringResource(R.string.edit_set_up), masked) {
        TextButton(onClick = { scope.launch { onChange() } }) {
            Text(stringResource(if (masked) R.string.edit_change else R.string.edit_replace))
        }
        TextButton(onClick = onRemove) { Text(stringResource(R.string.edit_remove)) }
    }
}

@Composable
private fun RemovedRow(label: String, onUndo: () -> Unit) {
    FieldRow(label, stringResource(R.string.edit_will_be_removed), masked = false) {
        TextButton(onClick = onUndo) { Text(stringResource(R.string.edit_undo)) }
    }
}

@Composable
private fun FieldRow(label: String, shown: String, masked: Boolean, buttons: @Composable () -> Unit) {
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (masked) {
                val hidden = stringResource(R.string.hidden, label)
                Text(
                    text = shown,
                    style = HavenType.masked,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    modifier = Modifier.clearAndSetSemantics { contentDescription = hidden },
                )
            } else {
                Text(
                    text = shown,
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        buttons()
    }
}
