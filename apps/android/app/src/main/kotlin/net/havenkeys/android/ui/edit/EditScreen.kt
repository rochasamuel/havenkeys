package net.havenkeys.android.ui.edit

import androidx.activity.compose.BackHandler
import androidx.annotation.StringRes
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.OfflineNote
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing
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

/** A fresh draft of [edit], knowing the vault's tag spellings; none while there is nothing to edit. */
private fun draftOf(edit: ItemEdit?, vaultTags: List<String>): EditorState? = edit?.let { EditorState(it, vaultTags) }

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
    val editor = remember(state.generation, edit) { draftOf(edit, state.vaultTags) }
    val loading = rememberTextLoads(editor, edit, viewModel)
    var confirmDiscard by remember { mutableStateOf(false) }
    val leave = { if (editor?.dirty == true) confirmDiscard = true else navigation.onBack() }
    val focus = LocalFocusManager.current
    val canSave = online && editor != null && loading.isEmpty() && !state.saving

    LaunchedEffect(viewModel) {
        viewModel.results.collect { if (it is EditResult.Saved) navigation.onDone(it.id) }
    }
    BackHandler(enabled = editor?.dirty == true) { confirmDiscard = true }

    HavenScaffold(
        modifier = modifier,
        topBar = {
            ScreenBar(onBack = leave, online = online, onLock = navigation.onLock) {
                HavenButton(
                    stringResource(R.string.edit_save),
                    onClick = {
                        // A tag still being typed joins the draft; one Rust would refuse
                        // keeps the editor open, its field saying why.
                        focus.clearFocus()
                        editor?.takeIf { it.commitTypedTag() }?.let { viewModel.save(it.toDraft()) }
                    },
                    // The bar pads 4dp: 12 more puts Save's edge on the fields' 16dp gutter.
                    Modifier.padding(start = 4.dp, end = 12.dp),
                    enabled = canSave,
                )
            }
        },
    ) { padding ->
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            edit?.let { LargeTitle(stringResource(screenTitle(it.kind, isNew))) }
            if (!online) OfflineNote(stringResource(R.string.edit_offline))
            state.errorCode?.let { ErrorLine(it) }
            if (editor != null && edit != null) {
                EditFields(editor, edit, FieldValues(viewModel, loading), state.vaultTags)
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
private fun DiscardDialog(isNew: Boolean, onKeep: () -> Unit, onDiscard: () -> Unit) {
    HavenDialog(
        title = stringResource(if (isNew) R.string.edit_discard_new else R.string.edit_discard_changes),
        onDismiss = onKeep,
        confirm = DialogAction(stringResource(R.string.edit_discard), onDiscard, danger = true),
        dismiss = DialogAction(stringResource(R.string.edit_keep_editing), onKeep),
    )
}

/** Not dismissible: the draft was made against a version that no longer exists; Reload is the only way on. */
@Composable
private fun ConflictDialog(onReload: () -> Unit) {
    HavenDialog(
        title = stringResource(R.string.error_item_changed_elsewhere),
        onDismiss = {},
        confirm = DialogAction(stringResource(R.string.edit_reload), onReload),
        message = stringResource(R.string.edit_conflict_text),
        dismissible = false,
    )
}
