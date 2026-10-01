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
