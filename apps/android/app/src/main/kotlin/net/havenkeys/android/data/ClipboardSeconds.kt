package net.havenkeys.android.data

/** How long a copied secret stays on the clipboard: the setting, else 30 s. */
suspend fun SettingsRepository.clipboardClearSeconds(): Int =
    (get() as? Outcome.Ok)?.value?.clipboardClearSeconds?.toInt() ?: DEFAULT_CLIPBOARD_SECONDS

// The core's default (havenkeys-core model.rs), used when the setting cannot be read.
private const val DEFAULT_CLIPBOARD_SECONDS = 30
