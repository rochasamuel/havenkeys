package net.havenkeys.android.ui.edit

import androidx.activity.compose.BackHandler
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.FieldKind
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

/** Where the editor leads; [onDone] carries only the saved item's id. */
class EditNavigation(
    val onDone: (id: String) -> Unit,
    val onBack: () -> Unit,
    val onLock: () -> Unit,
)

/** Where the fields get values from: Rust, through the ViewModel, never kept there. */
internal class FieldValues(val viewModel: EditViewModel, val loading: SnapshotStateList<String>)

/**
 * Create or edit one item. The draft ([EditorState]) is remembered here per
 * load generation and nowhere else: not in the ViewModel, not in saved
 * state (spec §9.4). Values are read from Rust only to be edited, one field
 * at a time.
 */
@Composable
fun EditScreen(
    viewModel: EditViewModel,
    isNew: Boolean,
    online: Boolean,
    navigation: EditNavigation,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val edit = state.edit
    val editor = remember(state.generation, edit) { edit?.let(::EditorState) }
    val loading = rememberTextLoads(editor, edit, viewModel)
    var confirmDiscard by remember { mutableStateOf(false) }
    val leave = { if (editor?.dirty == true) confirmDiscard = true else navigation.onBack() }
    val canSave = online && editor != null && loading.isEmpty() && !state.saving

    LaunchedEffect(viewModel) {
        viewModel.results.collect { if (it is EditResult.Saved) navigation.onDone(it.id) }
    }
    BackHandler(enabled = editor?.dirty == true) { confirmDiscard = true }

    Scaffold(
        topBar = {
            HavenTopBar(
                title = edit?.let { stringResource(screenTitle(it.kind, isNew)) }.orEmpty(),
                online = online,
                onLock = navigation.onLock,
                onBack = leave,
                actions = {
                    TextButton(onClick = { editor?.let { viewModel.save(it.toDraft()) } }, enabled = canSave) {
                        Text(stringResource(R.string.edit_save))
                    }
                },
            )
        },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .imePadding()
                .verticalScroll(rememberScrollState()),
        ) {
            if (!online) OfflineNote()
            state.errorCode?.let { ErrorLine(it) }
            if (editor != null && edit != null) {
                EditFields(editor, edit, FieldValues(viewModel, loading))
            }
        }
    }
    if (confirmDiscard) {
        DiscardDialog(isNew = isNew, onKeep = { confirmDiscard = false }, onDiscard = navigation.onBack)
    }
    if (state.conflict) ConflictDialog(onReload = viewModel::reload)
}

/**
 * The text values still on their way from Rust, shown as soon as they
 * arrive. Their fields stay disabled until then, so nothing typed is
 * measured against a value not yet loaded.
 */
@Composable
private fun rememberTextLoads(
    editor: EditorState?,
    edit: ItemEdit?,
    viewModel: EditViewModel,
): SnapshotStateList<String> {
    val loading = remember(editor) {
        val pending = edit?.fields.orEmpty().filter { it.present && it.kind == FieldKind.TEXT && it.value == null }
        mutableStateListOf<String>().apply { if (editor != null) addAll(pending.map { it.key }) }
    }
    LaunchedEffect(editor) {
        editor ?: return@LaunchedEffect
        for (key in loading.toList()) {
            (viewModel.reveal(key) as? Outcome.Ok)?.let { editor.load(key, it.value) }
            loading.remove(key)
        }
    }
    return loading
}

@StringRes
private fun screenTitle(kind: ItemKind, isNew: Boolean): Int = when (kind) {
    ItemKind.LOGIN -> if (isNew) R.string.edit_new_login else R.string.edit_login
    ItemKind.SECURE_NOTE -> if (isNew) R.string.edit_new_note else R.string.edit_note
    ItemKind.CARD -> if (isNew) R.string.edit_new_card else R.string.edit_card
    ItemKind.IDENTITY -> R.string.edit_identity
}

@Composable
private fun OfflineNote() {
    Surface(color = HavenTheme.colors.brassSoft, modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Outlined.CloudOff, contentDescription = null, tint = HavenTheme.colors.brassInk)
            Text(stringResource(R.string.edit_offline), style = MaterialTheme.typography.bodyMedium)
        }
    }
}

@Composable
private fun ErrorLine(code: String) {
    Text(
        text = stringResource(errorText(code)),
        color = MaterialTheme.colorScheme.error,
        style = MaterialTheme.typography.bodyMedium,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun DiscardDialog(isNew: Boolean, onKeep: () -> Unit, onDiscard: () -> Unit) {
    AlertDialog(
        onDismissRequest = onKeep,
        title = { Text(stringResource(if (isNew) R.string.edit_discard_new else R.string.edit_discard_changes)) },
        confirmButton = {
            TextButton(onClick = onDiscard) {
                Text(stringResource(R.string.edit_discard), color = MaterialTheme.colorScheme.error)
            }
        },
        dismissButton = { TextButton(onClick = onKeep) { Text(stringResource(R.string.edit_keep_editing)) } },
    )
}

/** Not dismissable: the draft was made against a version that no longer exists. */
@Composable
private fun ConflictDialog(onReload: () -> Unit) {
    AlertDialog(
        onDismissRequest = {},
        title = { Text(stringResource(R.string.error_item_changed_elsewhere)) },
        text = { Text(stringResource(R.string.edit_conflict_text)) },
        confirmButton = { TextButton(onClick = onReload) { Text(stringResource(R.string.edit_reload)) } },
    )
}
