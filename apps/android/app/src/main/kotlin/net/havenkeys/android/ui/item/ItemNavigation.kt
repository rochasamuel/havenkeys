package net.havenkeys.android.ui.item

/** Where the item screen leads; Edit carries only the item's id. */
class ItemNavigation(
    val onBack: () -> Unit,
    val onLock: () -> Unit,
    val onEdit: () -> Unit,
    val onDeleted: () -> Unit,
)
