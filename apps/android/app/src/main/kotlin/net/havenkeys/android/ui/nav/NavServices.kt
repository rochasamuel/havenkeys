package net.havenkeys.android.ui.nav

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.viewmodel.compose.viewModel
import net.havenkeys.android.AppContainer
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.ui.settings.SettingsActions
import net.havenkeys.android.ui.settings.rememberSettingsActions
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel

/**
 * What the app's navigation needs from the app. [rememberNavServices] builds
 * it from the AppContainer and the activity; a test builds it from fakes, so
 * the lock wipe runs through the real NavHost without Rust or a Keystore.
 * [unlockScreen] and [settingsActions] are the two places that need the
 * activity (biometric prompts).
 */
@Suppress("LongParameterList") // One property per thing the graph needs; grouping them would only hide them.
internal class NavServices(
    val vault: VaultRepository,
    val accounts: AccountRepository,
    val settings: SettingsRepository,
    val events: VaultEventsHub,
    val clipboard: SensitiveClipboard,
    val hasBiometricUnlock: () -> Boolean,
    val unlockScreen: @Composable (onUnlocked: () -> Unit) -> Unit,
    val settingsActions: @Composable () -> SettingsActions,
    val canVerifyUser: () -> Boolean,
    val verifyUser: suspend (title: String, subtitle: String) -> Boolean,
)

@Composable
internal fun rememberNavServices(container: AppContainer, activity: FragmentActivity): NavServices =
    remember(container, activity) {
        NavServices(
            vault = container.vaultRepository,
            accounts = container.accountRepository,
            settings = container.settingsRepository,
            events = container.events,
            clipboard = container.clipboard,
            hasBiometricUnlock = container::hasBiometricUnlock,
            unlockScreen = { onUnlocked ->
                UnlockScreen(
                    viewModel = viewModel {
                        UnlockViewModel(
                            container.vaultRepository,
                            biometricAvailable = container.biometricGate.available(activity),
                            hasBundle = container::hasBiometricUnlock,
                            deleteBundle = container::forgetBiometricUnlock,
                        )
                    },
                    activity = activity,
                    container = container,
                    onUnlocked = onUnlocked,
                )
            },
            settingsActions = { rememberSettingsActions(container, activity) },
            canVerifyUser = { container.biometricGate.canVerifyUser(activity) },
            verifyUser = { title, subtitle -> container.biometricGate.verifyUser(activity, title, subtitle) },
        )
    }
