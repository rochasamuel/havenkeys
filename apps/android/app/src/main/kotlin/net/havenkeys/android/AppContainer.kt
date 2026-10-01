package net.havenkeys.android

import java.io.IOException
import java.security.GeneralSecurityException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.launch
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.clipboard.clearClipboardOnLock
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.RustAccountRepository
import net.havenkeys.android.data.RustAutofillRepository
import net.havenkeys.android.data.RustSettingsRepository
import net.havenkeys.android.data.RustVaultRepository
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.security.BiometricGate
import net.havenkeys.android.security.BiometricKeys
import net.havenkeys.android.security.SecretKeyCipher
import uniffi.havenkeys_mobile.KeystoreCipher
import uniffi.havenkeys_mobile.MobileConfig
import uniffi.havenkeys_mobile.MobileVault

/** Manual wiring; one per process, so one vault for the app and its services. */
class AppContainer(app: HavenApp, cipher: KeystoreCipher) {
    val events = VaultEventsHub()
    private val vault = MobileVault(MobileConfig(app.filesDir.path, app.packageName), events, cipher)
    val vaultRepository: VaultRepository = RustVaultRepository(vault)
    val accountRepository: AccountRepository = RustAccountRepository(vault)
    val settingsRepository: SettingsRepository = RustSettingsRepository(vault)
    val autofillRepository: AutofillRepository = RustAutofillRepository(vault)
    val biometricKeys = BiometricKeys(app)
    val biometricGate = BiometricGate()
    val clipboard = SensitiveClipboard(app, app.appScope)

    /** Deletes the biometric key and bundle; a Keystore that does not answer is not an error here. */
    fun forgetBiometricUnlock() = keystoreDelete { biometricKeys.delete() }

    /** False too when the Keystore does not answer: the password still unlocks. */
    @Suppress("SwallowedException")
    fun hasBiometricUnlock(): Boolean = try {
        biometricKeys.hasBundle()
    } catch (e: GeneralSecurityException) {
        false
    }

    init {
        app.appScope.launch {
            wipeKeysOnExit(
                events.events,
                deleteBiometric = { keystoreDelete { biometricKeys.delete() } },
                deleteSecretKey = { keystoreDelete { SecretKeyCipher.delete() } },
            )
        }
        app.appScope.launch { clearClipboardOnLock(events.events, clipboard::clearIfOurs) }
    }
}

/**
 * Signing out or a refused bundle ends biometric unlock; removing the vault
 * also drops the Keystore key of the Secret Key file (spec §5.2, §6.4).
 */
internal suspend fun wipeKeysOnExit(
    events: Flow<VaultEvent>,
    deleteBiometric: () -> Unit,
    deleteSecretKey: () -> Unit,
) {
    events.collect { event ->
        when (event) {
            VaultEvent.Removed -> {
                deleteBiometric()
                deleteSecretKey()
            }
            VaultEvent.SignedOut -> deleteBiometric()
            is VaultEvent.Locked -> if (event.reason == BUNDLE_REFUSED) deleteBiometric()
            else -> Unit
        }
    }
}

private const val BUNDLE_REFUSED = "bundle_refused"

// A failed delete must not stop the collector: later events still need handling.
@Suppress("SwallowedException")
private inline fun keystoreDelete(delete: () -> Unit) {
    try {
        delete()
    } catch (e: GeneralSecurityException) {
        Unit
    } catch (e: IOException) {
        Unit
    }
}
