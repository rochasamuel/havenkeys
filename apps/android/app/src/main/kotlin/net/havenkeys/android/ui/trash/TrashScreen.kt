package net.havenkeys.android.ui.trash

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalResources
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.OfflineNote
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.DialogAction
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenDialog
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.ItemRow
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.shell.leading
import net.havenkeys.android.ui.shell.secondLine
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

private const val DAY_MILLIS = 86_400_000L

/** Whole days since [trashedAt] (Unix ms); a clock behind the server's reads as today. */
internal fun daysSince(trashedAt: Long, now: Long): Int =
    ((now - trashedAt) / DAY_MILLIS).toInt().coerceAtLeast(0)

/** What the Trash screen asks before deleting for good: one item, or all of them. */
private sealed interface TrashConfirm {
    data class Purge(val row: TrashRow) : TrashConfirm
    data class Empty(val count: Int, val passkeys: Boolean) : TrashConfirm
}

/**
 * The Trash (spec 2026-10-08-trash §6): deleted items, newest first, each
 * removed for good 30 days after it was deleted. Tapping one opens a sheet
 * with Restore and Delete permanently; Empty Trash deletes them all. A
 * trashed item shows its overview only: nothing is revealed, copied or
 * fetched. Every action writes to the server, so each waits for it.
 */
@Composable
fun TrashScreen(viewModel: TrashViewModel, online: Boolean, onBack: () -> Unit, onLock: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val toasts = rememberToastState()
    val resources = LocalResources.current
    // The open row's id only: its overview is looked up in the current list,
    // so a reload that drops it closes the sheet.
    var openId by rememberSaveable { mutableStateOf<String?>(null) }
    var confirm by remember { mutableStateOf<TrashConfirm?>(null) }
    val canAct = online && !state.busy

    HavenScaffold(
        topBar = {
            ScreenBar(onBack = onBack, online = online, onLock = onLock) {
                EmptyTrashMenu(
                    enabled = canAct && state.rows.isNotEmpty(),
                    onEmpty = {
                        confirm = TrashConfirm.Empty(state.rows.size, state.rows.any { it.summary.hasPasskey })
                    },
                )
            }
        },
        toastState = toasts,
    ) { padding ->
        TrashList(state, online, padding, onOpen = { openId = it })
    }

    val open = state.rows.firstOrNull { it.id == openId }
    LaunchedEffect(open == null, state.loading) { if (open == null && !state.loading) openId = null }
    if (open != null) {
        TrashSheet(
            open,
            state,
            online,
            onRestore = {
                viewModel.restore(open.id) {
                    toasts.show(resources.getString(R.string.trash_restored, open.summary.title))
                }
            },
            onDelete = { confirm = TrashConfirm.Purge(open) },
            onDismiss = { openId = null },
        )
    }
    TrashConfirmDialog(
        confirm,
        busy = state.busy,
        onConfirm = { asked ->
            confirm = null
            when (asked) {
                is TrashConfirm.Purge -> viewModel.purge(asked.row.id) {
                    openId = null
                    toasts.show(resources.getString(R.string.item_deleted, asked.row.summary.title))
                }
                is TrashConfirm.Empty -> viewModel.emptyTrash()
            }
        },
        onDismiss = { confirm = null },
    )
}

/**
 * Empty Trash behind More, as Delete is on an item: an ember menu row, and a
 * 48dp glyph in the bar that leaves room for the offline marker on a narrow
 * phone. It waits for the server and for something to empty.
 */
@Composable
private fun EmptyTrashMenu(enabled: Boolean, onEmpty: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    LaunchedEffect(enabled) { if (!enabled) open = false }
    Box {
        HavenIconButton(
            HavenIcon.More,
            stringResource(R.string.vault_more),
            onClick = { open = true },
            enabled = enabled,
        )
        HavenMenu(
            expanded = open && enabled,
            onDismiss = { open = false },
            items = listOf(
                MenuItem(stringResource(R.string.trash_empty_action), onEmpty, HavenIcon.Trash, danger = true),
            ),
        )
    }
}

@Composable
private fun TrashList(state: TrashUiState, online: Boolean, padding: PaddingValues, onOpen: (String) -> Unit) {
    val now = remember(state.rows) { System.currentTimeMillis() }
    LazyColumn(
        Modifier.fillMaxSize(),
        contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
    ) {
        item(key = "title") { LargeTitle(stringResource(R.string.trash_title), Gutter) }
        item(key = "explain") {
            HavenText(
                stringResource(R.string.trash_explain),
                Gutter.padding(bottom = 8.dp),
                color = HavenTheme.colors.muted,
            )
        }
        if (!online) item(key = "offline") { OfflineNote(stringResource(R.string.edit_offline), Gutter) }
        (state.errorCode ?: state.loadErrorCode)?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
        if (!state.loading && state.rows.isEmpty() && state.loadErrorCode == null) {
            item(key = "empty") { EmptyLine(stringResource(R.string.trash_empty), Gutter.padding(top = 8.dp)) }
        }
        if (state.rows.isNotEmpty()) item(key = "gap") { Spacer(Modifier.height(8.dp)) }
        insetGroup(state.rows, key = { it.id }) { row ->
            ItemRow(
                title = row.summary.title,
                subtitle = row.summary.secondLine(),
                leading = row.summary.leading(),
                onClick = { onOpen(row.id) },
                hasPasskey = row.summary.hasPasskey,
                hasCode = row.summary.hasTotp,
                note = trashNote(row, now),
            )
        }
    }
}

/** "Deleted 3 days ago · Removed in 27 days": when it went, and when it goes for good. */
@Composable
private fun trashNote(row: TrashRow, now: Long): String {
    val ago = daysSince(row.trashedAt, now)
    val deleted = when (ago) {
        0 -> stringResource(R.string.trash_deleted_today)
        1 -> stringResource(R.string.trash_deleted_yesterday)
        else -> pluralStringResource(R.plurals.trash_deleted_ago, ago, ago)
    }
    val removed = if (row.daysLeft <= 0) {
        stringResource(R.string.trash_removed_next_sync)
    } else {
        pluralStringResource(R.plurals.trash_removed_in, row.daysLeft, row.daysLeft)
    }
    return "$deleted · $removed"
}

/** One trashed item: its overview, why nothing else shows, and what can be done with it. */
@Suppress("LongParameterList") // the row, the screen's state, and its three answers
@Composable
private fun TrashSheet(
    row: TrashRow,
    state: TrashUiState,
    online: Boolean,
    onRestore: () -> Unit,
    onDelete: () -> Unit,
    onDismiss: () -> Unit,
) {
    HavenSheet(
        onDismiss = onDismiss,
        title = row.summary.title,
        footer = { TrashSheetActions(enabled = online && !state.busy, onRestore, onDelete) },
    ) {
        row.summary.secondLine()?.let { HavenText(it, color = HavenTheme.colors.muted) }
        HavenText(
            stringResource(R.string.trash_restore_first),
            Modifier.padding(top = 12.dp),
            color = HavenTheme.colors.text,
        )
        if (!online) OfflineNote(stringResource(R.string.edit_offline), Modifier.padding(top = 4.dp))
        state.errorCode?.let { ErrorLine(it) }
    }
}

@Composable
private fun ColumnScope.TrashSheetActions(enabled: Boolean, onRestore: () -> Unit, onDelete: () -> Unit) {
    Column(Modifier.fillMaxWidth().padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        HavenButton(
            stringResource(R.string.trash_restore),
            onClick = onRestore,
            Modifier.fillMaxWidth(),
            enabled = enabled,
        )
        HavenButton(
            stringResource(R.string.trash_delete_forever),
            onClick = onDelete,
            Modifier.fillMaxWidth(),
            style = ButtonStyle.Secondary,
            enabled = enabled,
        )
    }
}

/** The question before deleting for good; its button says the same verb as its text. */
@Composable
private fun TrashConfirmDialog(
    confirm: TrashConfirm?,
    busy: Boolean,
    onConfirm: (TrashConfirm) -> Unit,
    onDismiss: () -> Unit,
) {
    if (confirm == null) return
    val title = when (confirm) {
        is TrashConfirm.Purge -> stringResource(R.string.trash_confirm_delete, confirm.row.summary.title)
        is TrashConfirm.Empty -> pluralStringResource(R.plurals.trash_confirm_empty, confirm.count, confirm.count)
    }
    val passkeys = when (confirm) {
        is TrashConfirm.Purge -> confirm.row.summary.hasPasskey
        is TrashConfirm.Empty -> confirm.passkeys
    }
    HavenDialog(
        title = title,
        onDismiss = onDismiss,
        confirm = DialogAction(stringResource(R.string.trash_delete_forever), { onConfirm(confirm) }, danger = true),
        message = if (passkeys) stringResource(R.string.trash_passkey_warning) else null,
        dismiss = DialogAction(stringResource(R.string.item_cancel), onDismiss),
        busy = busy,
    )
}
