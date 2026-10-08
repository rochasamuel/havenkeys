package net.havenkeys.android.ui.item

/**
 * Where the item screen leads; Edit carries only the item's id. After a
 * Delete the screen goes: [onTrashed] when the item is in the Trash,
 * [onDeleted] when it was deleted for good. Both take only its title.
 */
class ItemNavigation(
    val onBack: () -> Unit,
    val onLock: () -> Unit,
    val onEdit: () -> Unit,
    val onTrashed: (title: String) -> Unit,
    val onDeleted: (title: String) -> Unit,
)
