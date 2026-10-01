package net.havenkeys.android.ui.settings

/** The navigation this screen leads to; none carries anything from the vault. */
class SettingsNavigation(
    val onBack: () -> Unit,
    val onLock: () -> Unit,
    val onDevices: () -> Unit,
    val onAutofillSetup: () -> Unit,
)
