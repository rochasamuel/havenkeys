package net.havenkeys.android.ui.nav

/** Navigation routes. An argument is never a secret: `item/{id}` carries only the item's UUID. */
object Routes {
    const val ONBOARDING = "onboarding"
    const val UNLOCK = "unlock"
    const val VAULT = "vault"
    const val ITEM = "item/{id}"
    const val GENERATOR = "generator"
    const val SETTINGS = "settings"
    const val DEVICES = "devices"
    const val AUTOFILL_SETUP = "autofill-setup"
}

internal fun routeOf(start: Start): String = when (start) {
    Start.ONBOARDING -> Routes.ONBOARDING
    Start.UNLOCK -> Routes.UNLOCK
    Start.VAULT -> Routes.VAULT
}

/**
 * After a process kill the navigation state comes back (say, an item) while
 * the new process is locked and no lock event will come. Returns the route
 * to replace the whole back stack with, or null when [current] may stay.
 */
internal fun routeToForce(current: String?, start: Start): String? =
    if (start != Start.VAULT && current != routeOf(start)) routeOf(start) else null
