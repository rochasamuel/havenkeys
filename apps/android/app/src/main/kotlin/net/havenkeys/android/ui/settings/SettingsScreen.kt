package net.havenkeys.android.ui.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.outlined.KeyboardArrowRight
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.enrollBiometrics
import uniffi.havenkeys_mobile.MobileSettings

@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel,
    activity: FragmentActivity,
    container: AppContainer,
    online: Boolean,
    navigation: SettingsNavigation,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    var dialog by remember { mutableStateOf<SettingsDialog?>(null) }
    val biometricAvailable = remember(activity) { container.biometricGate.available(activity) }

    Scaffold(
        topBar = {
            HavenTopBar(
                stringResource(R.string.settings_title),
                online = online,
                onLock = navigation.onLock,
                onBack = navigation.onBack,
            )
        },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        Column(
            Modifier
                .padding(padding)
                .fillMaxSize()
                .verticalScroll(rememberScrollState()),
        ) {
            state.errorCode?.let { SettingsError(it) }
            state.settings?.let { settings ->
                SecuritySection(
                    settings = settings,
                    viewModel = viewModel,
                    biometricEnrolled = state.biometricEnrolled,
                    biometricAvailable = biometricAvailable,
                    onBiometric = { on ->
                        if (on) {
                            dialog = SettingsDialog.BIOMETRIC_PASSWORD
                        } else {
                            container.forgetBiometricUnlock()
                            viewModel.biometricChanged()
                        }
                    },
                )
                AutofillSection(settings, viewModel, navigation.onAutofillSetup)
            }
            AccountSection(
                online = online,
                onDevices = navigation.onDevices,
                onSignOut = { dialog = SettingsDialog.SIGN_OUT },
                onRemove = { dialog = SettingsDialog.REMOVE },
            )
        }
    }

    SettingsDialogs(
        dialog = dialog,
        state = state,
        viewModel = viewModel,
        onClose = { dialog = null },
        // The composition's scope: main thread, as BiometricPrompt requires.
        onPassword = { password ->
            scope.launch { viewModel.biometricChanged(enrollBiometrics(activity, container, password)) }
        },
    )
}

/** A refused value (`invalid_input`) reads as the desktop's "Could not save settings." */
@Composable
private fun SettingsError(code: String) {
    Text(
        stringResource(if (code == INVALID_INPUT) R.string.settings_save_failed else errorText(code)),
        color = MaterialTheme.colorScheme.error,
        style = MaterialTheme.typography.bodyMedium,
        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
    )
}

@Composable
private fun SecuritySection(
    settings: MobileSettings,
    viewModel: SettingsViewModel,
    biometricEnrolled: Boolean,
    biometricAvailable: Boolean,
    onBiometric: (Boolean) -> Unit,
) {
    SectionTitle(R.string.settings_security)
    ChoiceRow(
        label = R.string.settings_auto_lock,
        choices = SettingsViewModel.AUTO_LOCK_CHOICES,
        selected = settings.autoLockMinutes,
        text = { autoLockText(it) },
        onSelect = viewModel::setAutoLock,
    )
    ChoiceRow(
        label = R.string.settings_clipboard,
        choices = (SettingsViewModel.CLIPBOARD_CHOICES + settings.clipboardClearSeconds).distinct().sorted(),
        selected = settings.clipboardClearSeconds,
        text = { stringResource(R.string.settings_after_seconds, it.toInt()) },
        onSelect = viewModel::setClipboardSeconds,
    )
    SwitchRow(R.string.settings_lock_on_screen_off, settings.lockOnScreenOff, viewModel::setLockOnScreenOff)
    SwitchRow(
        label = R.string.settings_biometric,
        checked = biometricEnrolled,
        onChange = onBiometric,
        // Turning off always works; turning on needs a strong biometric enrolled.
        enabled = biometricEnrolled || biometricAvailable,
        note = if (biometricEnrolled || biometricAvailable) {
            R.string.settings_biometric_note
        } else {
            R.string.error_biometric_unavailable
        },
    )
}

@Composable
private fun AutofillSection(settings: MobileSettings, viewModel: SettingsViewModel, onAutofillSetup: () -> Unit) {
    SectionTitle(R.string.settings_autofill)
    LinkRow(R.string.settings_autofill_setup, onAutofillSetup)
    SwitchRow(
        label = R.string.settings_confirm_before_filling,
        checked = settings.confirmBeforeFilling,
        onChange = viewModel::setConfirmBeforeFilling,
        note = R.string.settings_confirm_before_filling_note,
    )
    SwitchRow(
        label = R.string.settings_asset_links,
        checked = settings.assetLinks,
        onChange = viewModel::setAssetLinks,
        note = R.string.settings_asset_links_note,
    )
}

@Composable
private fun AccountSection(online: Boolean, onDevices: () -> Unit, onSignOut: () -> Unit, onRemove: () -> Unit) {
    SectionTitle(R.string.settings_account)
    LinkRow(R.string.settings_devices, onDevices)
    // Offline, signing out still locks; only the server's session end waits.
    LinkRow(R.string.settings_sign_out, onSignOut, note = if (online) null else R.string.error_offline)
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant)
    LinkRow(
        R.string.settings_remove_title,
        onRemove,
        note = if (online) R.string.settings_remove_note else R.string.error_offline,
        danger = true,
    )
}

@Composable
private fun autoLockText(minutes: UInt): String = when (minutes) {
    0u -> stringResource(R.string.settings_never)
    MINUTES_PER_HOUR -> stringResource(R.string.settings_after_hour)
    else -> stringResource(R.string.settings_after_minutes, minutes.toInt())
}

@Composable
private fun SectionTitle(@StringRes text: Int) {
    Text(
        stringResource(text),
        style = MaterialTheme.typography.titleSmall,
        color = HavenTheme.colors.textStrong,
        modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 20.dp, bottom = 4.dp),
    )
}

@Composable
private fun SwitchRow(
    @StringRes label: Int,
    checked: Boolean,
    onChange: (Boolean) -> Unit,
    enabled: Boolean = true,
    @StringRes note: Int? = null,
) {
    ListItem(
        headlineContent = { Text(stringResource(label)) },
        supportingContent = note?.let { { Text(stringResource(it)) } },
        trailingContent = { Switch(checked = checked, onCheckedChange = null, enabled = enabled) },
        colors = rowColors(),
        modifier = Modifier.selectable(
            selected = checked,
            enabled = enabled,
            role = Role.Switch,
            onClick = { onChange(!checked) },
        ),
    )
}

@Composable
private fun LinkRow(@StringRes label: Int, onClick: () -> Unit, @StringRes note: Int? = null, danger: Boolean = false) {
    ListItem(
        headlineContent = {
            Text(
                stringResource(label),
                color = if (danger) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface,
            )
        },
        supportingContent = note?.let { { Text(stringResource(it)) } },
        trailingContent = { Icon(Icons.AutoMirrored.Outlined.KeyboardArrowRight, contentDescription = null) },
        colors = rowColors(),
        modifier = Modifier.clickable(onClick = onClick),
    )
}

@Composable
internal fun rowColors() = ListItemDefaults.colors(containerColor = MaterialTheme.colorScheme.background)

internal const val INVALID_INPUT = "invalid_input"
private const val MINUTES_PER_HOUR = 60u
