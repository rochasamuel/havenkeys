package net.havenkeys.android.ui.edit

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.input.TextFieldLineLimits
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
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
import androidx.compose.ui.text.input.KeyboardType
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.MaskedValue
import net.havenkeys.android.ui.item.fieldLabel
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.HavenTextField
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.SecretTextField
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.EditField
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

/** Long text the user edits: multi-line, and not masked once opened. */
private val longText = setOf("notes", "content", "card.notes", "identity.notes")

private const val LONG_TEXT_LINES = 3

@Composable
internal fun EditFields(editor: EditorState, edit: ItemEdit, values: FieldValues, vaultTags: List<String>) {
    Column(verticalArrangement = Arrangement.spacedBy(HavenSpacing.groupGap)) {
        // The identity's title is its name, built by Rust.
        if (edit.kind != ItemKind.IDENTITY) InsetGroup { row { TitleEditor(editor) } }
        if (edit.kind == ItemKind.LOGIN) Websites(editor)
        if (edit.fields.isNotEmpty()) {
            InsetGroup {
                edit.fields.forEach { field -> row { key(field.key) { FieldEditor(field, editor, values) } } }
            }
        }
        if (edit.hasCustomFields) {
            HavenText(
                stringResource(R.string.edit_custom_kept),
                Modifier.padding(horizontal = HavenSpacing.rowX),
                color = HavenTheme.colors.muted,
            )
        }
        Tags(editor, vaultTags)
    }
}

@Composable
private fun TitleEditor(editor: EditorState) {
    val title = rememberDraftText(editor, initial = { editor.title }, onEdit = { editor.title = it })
    HavenTextField(title, stringResource(R.string.edit_title))
}

@Composable
private fun FieldEditor(field: EditField, editor: EditorState, values: FieldValues) {
    val key = field.key
    val label = stringResource(fieldLabel(key))
    // Removal first: the old value is never shown for a field being removed.
    when {
        editor.isRemoved(key) -> RemovedRow(label, onUndo = { editor.undo(key) })
        field.kind == FieldKind.TEXT -> TextEditor(key, label, editor, values)
        field.kind == FieldKind.SECRET && field.present && !editor.isOpen(key) -> HiddenRow(
            label = label,
            masked = true,
            onChange = { loadSecret(key, editor, values) },
            onRemove = { editor.remove(key) },
        )
        field.kind == FieldKind.SECRET -> SecretEditor(key, label, editor, values)
        // A one-time code's key is never shown: it can be replaced or removed.
        field.present && !editor.isOpen(key) -> HiddenRow(
            label = label,
            masked = false,
            onChange = { editor.open(key) },
            onRemove = { editor.remove(key) },
        )
        else -> SetupKeyEditor(key, label, editor, values)
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
    val loading = key in values.loading
    // Keyed on `loading`: Rust's value replaces the field that waited for it, disabled until then.
    val text = rememberDraftText(
        editor,
        key,
        loading,
        initial = { editor.shown(key) },
        onEdit = { editor.type(key, it) },
    )
    HavenTextField(
        text,
        label,
        enabled = !loading,
        placeholder = if (key == "card.expiry") stringResource(R.string.edit_expiry_placeholder) else null,
        keyboardOptions = KeyboardOptions(
            keyboardType = KeyboardType.Text,
            autoCorrectEnabled = !(key == "card.holder" || key.startsWith("identity.") || key == "username"),
        ),
        lineLimits = if (key in longText) {
            TextFieldLineLimits.MultiLine(minHeightInLines = LONG_TEXT_LINES)
        } else {
            TextFieldLineLimits.SingleLine
        },
    )
}

@Composable
private fun SecretEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val text = rememberDraftText(editor, key, initial = { editor.shown(key) }, onEdit = { editor.type(key, it) })
    if (key in longText) {
        // A secure note's content: long text, shown while it is edited, as before.
        HavenTextField(
            text,
            label,
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false),
            lineLimits = TextFieldLineLimits.MultiLine(minHeightInLines = LONG_TEXT_LINES),
        )
        return
    }
    val scope = rememberCoroutineScope()
    var visible by remember { mutableStateOf(false) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        SecretTextField(
            text,
            label,
            revealed = visible,
            onRevealChange = { visible = it },
            modifier = Modifier.weight(1f),
        )
        if (key == "password") {
            HavenIconButton(
                HavenIcon.Dice,
                stringResource(R.string.edit_generate),
                onClick = {
                    scope.launch {
                        val generated = values.viewModel.generate()
                        if (generated is Outcome.Ok) {
                            // Into the field, which hands it to the draft like typing would.
                            text.setTextAndPlaceCursorAtEnd(generated.value)
                            visible = true
                        }
                    }
                },
            )
        }
    }
}

/**
 * A one-time code's setup key or otpauth:// link: a secret, typed masked; the
 * eye shows it. The QR button scans the code the site shows instead, and the
 * link it finds goes in as if typed.
 */
@Composable
private fun SetupKeyEditor(key: String, label: String, editor: EditorState, values: FieldValues) {
    val text = rememberDraftText(editor, key, initial = { editor.shown(key) }, onEdit = { editor.type(key, it) })
    var visible by remember { mutableStateOf(false) }
    var scanning by remember { mutableStateOf(false) }
    var scanned by remember { mutableStateOf(false) }
    SecretTextField(
        text,
        label,
        revealed = visible,
        onRevealChange = { visible = it },
        hint = stringResource(if (scanned) R.string.edit_totp_scanned else R.string.edit_totp_placeholder),
        action = {
            HavenIconButton(HavenIcon.Qr, stringResource(R.string.edit_totp_scan), onClick = { scanning = true })
        },
    )
    if (scanning) {
        TotpScanSheet(
            scan = values.viewModel::scanTotp,
            onFound = { link ->
                text.setTextAndPlaceCursorAtEnd(link)
                scanned = true
                scanning = false
            },
            onDismiss = { scanning = false },
        )
    }
}

/** A present value that is not shown: the fixed mask (never its length) or "Set up.", with Change and Remove. */
@Composable
private fun HiddenRow(label: String, masked: Boolean, onChange: suspend () -> Unit, onRemove: () -> Unit) {
    val scope = rememberCoroutineScope()
    GroupRow(
        trailing = {
            HavenButton(
                stringResource(if (masked) R.string.edit_change else R.string.edit_replace),
                onClick = { scope.launch { onChange() } },
                style = ButtonStyle.Quiet,
            )
            HavenButton(stringResource(R.string.edit_remove), onClick = onRemove, style = ButtonStyle.Quiet)
        },
    ) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        if (masked) {
            // The label is read just above: the dots say only "Hidden".
            MaskedValue(label = null)
        } else {
            HavenText(
                stringResource(R.string.edit_set_up),
                style = HavenTheme.type.value,
                color = HavenTheme.colors.muted,
            )
        }
    }
}

@Composable
private fun RemovedRow(label: String, onUndo: () -> Unit) {
    GroupRow(
        trailing = { HavenButton(stringResource(R.string.edit_undo), onClick = onUndo, style = ButtonStyle.Quiet) },
    ) {
        HavenText(label, style = HavenTheme.type.label, color = HavenTheme.colors.muted)
        HavenText(
            stringResource(R.string.edit_will_be_removed),
            style = HavenTheme.type.value,
            color = HavenTheme.colors.muted,
        )
    }
}
