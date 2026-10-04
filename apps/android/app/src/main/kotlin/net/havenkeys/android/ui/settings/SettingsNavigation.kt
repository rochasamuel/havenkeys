package net.havenkeys.android.ui.settings

/** Where Settings leads; none carries anything from the vault. The shell's top bar has Lock. */
class SettingsNavigation(val onDevices: () -> Unit, val onAutofillSetup: () -> Unit, val onPairing: () -> Unit)
