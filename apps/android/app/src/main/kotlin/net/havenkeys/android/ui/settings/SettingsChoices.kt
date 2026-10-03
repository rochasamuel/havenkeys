@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ChoiceSheet
import uniffi.havenkeys_mobile.MobileSettings

/** The settings chosen from a short list. */
enum class SettingsChoice { AUTO_LOCK, CLIPBOARD }

private const val MINUTES_PER_HOUR = 60u

@Composable
internal fun autoLockText(minutes: UInt): String = when (minutes) {
    0u -> stringResource(R.string.settings_never)
    MINUTES_PER_HOUR -> stringResource(R.string.settings_after_hour)
    else -> stringResource(R.string.settings_after_minutes, minutes.toInt())
}

@Composable
internal fun clipboardText(seconds: UInt): String = stringResource(R.string.settings_after_seconds, seconds.toInt())

/** The sheet for [choice]; picking a value saves it and closes the sheet. */
@Composable
internal fun SettingsChoiceSheet(
    choice: SettingsChoice,
    settings: MobileSettings,
    viewModel: SettingsViewModel,
    onClose: () -> Unit,
) {
    when (choice) {
        SettingsChoice.AUTO_LOCK -> ChoiceSheet(
            title = stringResource(R.string.settings_auto_lock),
            values = SettingsViewModel.AUTO_LOCK_CHOICES,
            selected = settings.autoLockMinutes,
            label = { autoLockText(it) },
            onSelect = viewModel::setAutoLock,
            onDismiss = onClose,
        )
        SettingsChoice.CLIPBOARD -> ChoiceSheet(
            title = stringResource(R.string.settings_clipboard),
            // A value set on the desktop outside the phone's list still shows, selected.
            values = (SettingsViewModel.CLIPBOARD_CHOICES + settings.clipboardClearSeconds).distinct().sorted(),
            selected = settings.clipboardClearSeconds,
            label = { clipboardText(it) },
            onSelect = viewModel::setClipboardSeconds,
            onDismiss = onClose,
        )
    }
}
