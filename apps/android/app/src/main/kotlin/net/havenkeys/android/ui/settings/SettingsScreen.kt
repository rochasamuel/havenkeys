package net.havenkeys.android.ui.settings

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.PRICING_URL
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.components.openLink
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.InsetGroupScope
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.ToggleRow
import net.havenkeys.android.ui.kit.TrailingText
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.MobileSettings

/** Rust's code for a refused value; it reads as the desktop's "Could not save settings." */
internal const val INVALID_INPUT = "invalid_input"

/** Opens a choice sheet or a dialog; one handle so the groups stay short. */
private class SettingsOpen(val choose: (SettingsChoice) -> Unit, val dialog: (SettingsDialog) -> Unit)

/**
 * The Settings tab (spec §6.8): today's settings as grouped rows. The
 * shell's top bar above it has Lock and Sync now; Devices, Autofill
 * setup and the Trash open full screen over the shell.
 */
@Composable
fun SettingsScreen(
    viewModel: SettingsViewModel,
    online: Boolean,
    actions: SettingsActions,
    navigation: SettingsNavigation,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    val verifyTitle = stringResource(R.string.settings_delete_verify_title)
    var dialog by remember { mutableStateOf<SettingsDialog?>(null) }
    var choosing by remember { mutableStateOf<SettingsChoice?>(null) }
    val open = remember { SettingsOpen(choose = { choosing = it }, dialog = { dialog = it }) }
    Column(
        modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = HavenSpacing.gutter)
            .padding(top = 8.dp, bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
    ) {
        LargeTitle(stringResource(R.string.settings_title))
        state.errorCode?.let { SettingsError(it) }
        state.settings?.let { settings ->
            SecurityGroup(settings, state.biometricEnrolled, actions, viewModel, open)
            AutofillGroup(settings, viewModel, navigation.onAutofillSetup)
        }
        VaultGroup(state.trashCount, navigation.onTrash)
        AccountGroup(state, online, navigation, open)
    }
    val settings = state.settings
    val choice = choosing
    if (settings != null && choice != null) {
        SettingsChoiceSheet(choice, settings, viewModel, onClose = { choosing = null })
    }
    SettingsDialogs(
        dialog = dialog,
        viewModel = viewModel,
        onClose = { dialog = null },
        // The composition's scope: main thread, as BiometricPrompt requires.
        onPassword = { password -> scope.launch { viewModel.biometricChanged(actions.enrollBiometric(password)) } },
        onDeleteAccount = { email, password ->
            scope.launch { if (actions.verifyUser(verifyTitle)) viewModel.deleteAccount(email, password) }
        },
    )
}

@Composable
private fun SettingsError(code: String) {
    HavenText(
        stringResource(if (code == INVALID_INPUT) R.string.settings_save_failed else errorText(code)),
        Modifier.padding(vertical = 8.dp),
        color = HavenTheme.colors.danger,
    )
}

@Composable
private fun SecurityGroup(
    settings: MobileSettings,
    enrolled: Boolean,
    actions: SettingsActions,
    viewModel: SettingsViewModel,
    open: SettingsOpen,
) {
    // Turning off always works; turning on needs a strong biometric enrolled.
    val canEnroll = enrolled || actions.biometricAvailable
    SectionHeader(stringResource(R.string.settings_security), Modifier.padding(top = 8.dp))
    InsetGroup {
        row {
            GroupRow(onClick = { open.choose(SettingsChoice.AUTO_LOCK) }) {
                GroupRowText(stringResource(R.string.settings_auto_lock), autoLockText(settings.autoLockMinutes))
            }
        }
        row {
            GroupRow(onClick = { open.choose(SettingsChoice.CLIPBOARD) }) {
                GroupRowText(stringResource(R.string.settings_clipboard), clipboardText(settings.clipboardClearSeconds))
            }
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_lock_on_screen_off),
                settings.lockOnScreenOff,
                viewModel::setLockOnScreenOff,
            )
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_biometric),
                checked = enrolled,
                onCheckedChange = { on ->
                    if (on) {
                        open.dialog(SettingsDialog.BIOMETRIC_PASSWORD)
                    } else {
                        actions.forgetBiometric()
                        viewModel.biometricChanged()
                    }
                },
                detail = stringResource(
                    if (canEnroll) R.string.settings_biometric_note else R.string.error_biometric_unavailable,
                ),
                enabled = canEnroll,
            )
        }
    }
}

@Composable
private fun AutofillGroup(settings: MobileSettings, viewModel: SettingsViewModel, onAutofillSetup: () -> Unit) {
    SectionHeader(stringResource(R.string.settings_autofill), Modifier.padding(top = 16.dp))
    InsetGroup {
        row { GroupRow(onClick = onAutofillSetup) { GroupRowText(stringResource(R.string.settings_autofill_setup)) } }
        row {
            ToggleRow(
                stringResource(R.string.settings_confirm_before_filling),
                settings.confirmBeforeFilling,
                viewModel::setConfirmBeforeFilling,
                detail = stringResource(R.string.settings_confirm_before_filling_note),
            )
        }
        row {
            ToggleRow(
                stringResource(R.string.settings_asset_links),
                settings.assetLinks,
                viewModel::setAssetLinks,
                detail = stringResource(R.string.settings_asset_links_note),
            )
        }
    }
}

/** The Trash, with how many items wait in it; it opens full screen, as Devices does. */
@Composable
private fun VaultGroup(trashCount: Int?, onTrash: () -> Unit) {
    SectionHeader(stringResource(R.string.settings_vault), Modifier.padding(top = 16.dp))
    InsetGroup {
        row {
            GroupRow(
                onClick = onTrash,
                trailing = { if (trashCount != null) TrailingText(trashCount.toString()) },
                chevron = true,
            ) { GroupRowText(stringResource(R.string.trash_title)) }
        }
    }
}

@Composable
private fun AccountGroup(state: SettingsUiState, online: Boolean, navigation: SettingsNavigation, open: SettingsOpen) {
    val email = state.email
    val offline = if (online) null else stringResource(R.string.error_offline)
    SectionHeader(stringResource(R.string.settings_account), Modifier.padding(top = 16.dp))
    InsetGroup {
        if (email != null) row { GroupRow { GroupRowText(stringResource(R.string.settings_signed_in_as), email) } }
        planRows(state)
        row { GroupRow(onClick = navigation.onDevices) { GroupRowText(stringResource(R.string.settings_devices)) } }
        row {
            GroupRow(onClick = navigation.onPairing) {
                GroupRowText(stringResource(R.string.settings_pairing), offline)
            }
        }
        // Offline, signing out still locks; only the server's session end waits.
        row {
            GroupRow(onClick = { open.dialog(SettingsDialog.SIGN_OUT) }) {
                GroupRowText(stringResource(R.string.settings_sign_out), offline)
            }
        }
    }
    Spacer(Modifier.height(HavenSpacing.groupGap))
    InsetGroup {
        row {
            GroupRow(onClick = { open.dialog(SettingsDialog.REMOVE) }) {
                HavenText(
                    stringResource(R.string.settings_remove_title),
                    style = HavenTheme.type.value,
                    color = HavenTheme.colors.danger,
                )
                HavenText(
                    offline ?: stringResource(R.string.settings_remove_note),
                    style = HavenTheme.type.rowSubtitle,
                    color = HavenTheme.colors.muted,
                )
            }
        }
        row {
            GroupRow(onClick = { if (online) open.dialog(SettingsDialog.DELETE_ACCOUNT) }) {
                HavenText(
                    stringResource(R.string.settings_delete_title),
                    style = HavenTheme.type.value,
                    color = HavenTheme.colors.danger,
                )
                HavenText(
                    offline ?: stringResource(R.string.settings_delete_note),
                    style = HavenTheme.type.rowSubtitle,
                    color = HavenTheme.colors.muted,
                )
            }
        }
    }
}

/** The plan row (and, when frozen, the way to subscribe); Rust decides, this only displays. */
private fun InsetGroupScope.planRows(state: SettingsUiState) {
    val planStatus = state.planStatus
    val daysLeft = state.trialEndsAt?.let { trialDaysLeft(it) }
    val plain = when {
        state.frozen -> R.string.settings_plan_frozen
        planStatus == "active" -> R.string.settings_plan_active
        planStatus == "past_due" -> R.string.settings_plan_past_due
        else -> null
    }
    val trialDays = daysLeft.takeIf { !state.frozen && planStatus == "trialing" }
    // Complimentary, or a status this app does not know: no row.
    if (plain != null || trialDays != null) {
        row {
            val detail = if (plain != null) {
                stringResource(plain)
            } else {
                val days = trialDays ?: 0
                pluralStringResource(R.plurals.settings_plan_trial, days, days)
            }
            GroupRow { GroupRowText(stringResource(R.string.settings_plan), detail) }
        }
    }
    if (state.frozen) {
        row {
            val context = LocalContext.current
            GroupRow(onClick = { openLink(context, PRICING_URL) }) {
                GroupRowText(stringResource(R.string.settings_subscribe))
            }
        }
    }
}
