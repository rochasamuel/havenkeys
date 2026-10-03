@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenPress
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
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
            Choices(
                stringResource(R.string.settings_auto_lock),
                SettingsViewModel.AUTO_LOCK_CHOICES,
                settings.autoLockMinutes,
            ) { autoLockText(it) },
            onSelect = viewModel::setAutoLock,
            onClose = onClose,
        )
        SettingsChoice.CLIPBOARD -> ChoiceSheet(
            Choices(
                stringResource(R.string.settings_clipboard),
                // A value set on the desktop outside the phone's list still shows, selected.
                (SettingsViewModel.CLIPBOARD_CHOICES + settings.clipboardClearSeconds).distinct().sorted(),
                settings.clipboardClearSeconds,
            ) { clipboardText(it) },
            onSelect = viewModel::setClipboardSeconds,
            onClose = onClose,
        )
    }
}

private class Choices(
    val title: String,
    val values: List<UInt>,
    val selected: UInt,
    val text: @Composable (UInt) -> String,
)

@Composable
private fun ChoiceSheet(choices: Choices, onSelect: (UInt) -> Unit, onClose: () -> Unit) {
    HavenSheet(onDismiss = onClose, title = choices.title) {
        InsetGroup(Modifier.selectableGroup()) {
            choices.values.forEach { value ->
                row {
                    ChoiceOption(choices.text(value), selected = value == choices.selected) {
                        onClose()
                        if (value != choices.selected) onSelect(value)
                    }
                }
            }
        }
    }
}

/** One value: a radio button for TalkBack, a brass check when it is the current one. */
@Composable
private fun ChoiceOption(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = HavenTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.rowMin)
            .selectable(
                selected = selected,
                interactionSource = null,
                indication = HavenPress,
                role = Role.RadioButton,
                onClick = onClick,
            )
            .padding(horizontal = HavenSpacing.rowX, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenText(label, Modifier.weight(1f), style = HavenTheme.type.value, color = colors.textStrong)
        if (selected) IconGlyph(HavenIcon.Check, contentDescription = null, tint = colors.brass, size = 20.dp)
    }
}
