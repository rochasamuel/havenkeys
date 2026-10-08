package net.havenkeys.android.ui.shell

import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalResources
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.ToastAction
import net.havenkeys.android.ui.kit.ToastState
import net.havenkeys.android.ui.kit.ToastTone

/**
 * The last item Delete took away, for the screen under the item to say so
 * once: the item screen pops back as it deletes, so its own toast would go
 * with it. [restore] puts a trashed item back (Undo). Title and id only,
 * never a value.
 */
@Stable
class TrashUndo(private val restore: suspend (id: String) -> Outcome<Unit>) {
    var pending: Trashed? by mutableStateOf(null)
        private set

    /** The item is in the Trash: the toast offers Undo. */
    fun offer(id: String, title: String) {
        pending = Trashed(id, title, inTrash = true)
    }

    /** The item was deleted for good (its details did not open): the toast only says so. */
    fun deletedForGood(id: String, title: String) {
        pending = Trashed(id, title, inTrash = false)
    }

    fun take(): Trashed? = pending.also { pending = null }

    internal suspend fun undo(id: String): Outcome<Unit> = restore(id)
}

@Immutable
data class Trashed(val id: String, val title: String, val inTrash: Boolean)

/**
 * Shows [undo]'s pending delete in [toasts] once, on whichever screen an
 * item was opened from (the shell, search, vault health). Undo restores
 * the item; a failed restore says why in the app's words.
 */
@Composable
fun TrashUndoToast(undo: TrashUndo?, toasts: ToastState) {
    if (undo == null) return
    val resources = LocalResources.current
    val scope = rememberCoroutineScope()
    LaunchedEffect(undo, undo.pending) {
        val trashed = undo.take() ?: return@LaunchedEffect
        if (!trashed.inTrash) {
            toasts.show(resources.getString(R.string.item_deleted, trashed.title))
            return@LaunchedEffect
        }
        val action = ToastAction(resources.getString(R.string.edit_undo)) {
            scope.launch {
                val r = undo.undo(trashed.id)
                if (r is Outcome.Failed) toasts.show(resources.getString(errorText(r.code)), ToastTone.Alert)
            }
        }
        toasts.show(resources.getString(R.string.trash_moved, trashed.title), ToastTone.Done, action)
    }
}
