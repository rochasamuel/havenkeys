package net.havenkeys.android.ui.nav

import uniffi.havenkeys_mobile.ItemKind

/**
 * The app's routes. An argument is never a secret: `item/{id}` and
 * `edit/{id}` carry only the item's UUID, `new/{kind}` only a kind;
 * `search` takes none (the query lives only in its ViewModel). The tabs
 * and category lists are routes of the shell's own NavHost (ShellRoutes).
 */
object Routes {
    const val ONBOARDING = "onboarding"
    const val UNLOCK = "unlock"

    /** The shell: top bar, the tabs, bottom bar (spec §6.1). The tabs have routes of their own inside it. */
    const val SHELL = "shell"

    /** Search takes no argument: the query lives only in its ViewModel. */
    const val SEARCH = "search"

    const val ITEM_ID = "id"
    const val ITEM = "item/{$ITEM_ID}"
    const val GENERATOR = "generator"
    const val DEVICES = "devices"
    const val PAIRING = "pairing"
    const val AUTOFILL_SETUP = "autofill-setup"
    const val HEALTH = "health"
    const val TRASH = "trash"
    const val EDIT = "edit/{$ITEM_ID}"
    const val KIND = "kind"
    const val NEW = "new/{$KIND}"

    fun item(id: String) = "item/$id"
    fun edit(id: String) = "edit/$id"
    fun new(kind: ItemKind) = "new/${kindArg(kind)}"
}

private fun kindArg(kind: ItemKind): String = when (kind) {
    ItemKind.LOGIN -> "login"
    ItemKind.SECURE_NOTE -> "note"
    ItemKind.CARD -> "card"
    ItemKind.IDENTITY -> "identity"
}

/** The kinds the phone can create; the identity is never one of them. */
internal fun creatableKind(arg: String): ItemKind? = when (arg) {
    "login" -> ItemKind.LOGIN
    "note" -> ItemKind.SECURE_NOTE
    "card" -> ItemKind.CARD
    else -> null
}

internal fun routeOf(start: Start): String = when (start) {
    Start.ONBOARDING -> Routes.ONBOARDING
    Start.UNLOCK -> Routes.UNLOCK
    Start.VAULT -> Routes.SHELL
}

/**
 * After a process kill the navigation state comes back (say, an item) while
 * the new process is locked and no lock event will come. Returns the route
 * to replace the whole back stack with, or null when [current] may stay.
 */
internal fun routeToForce(current: String?, start: Start): String? =
    if (start != Start.VAULT && current != routeOf(start)) routeOf(start) else null
