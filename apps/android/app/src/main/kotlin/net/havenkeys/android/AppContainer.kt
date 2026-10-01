package net.havenkeys.android

import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.RustAccountRepository
import net.havenkeys.android.data.RustAutofillRepository
import net.havenkeys.android.data.RustSettingsRepository
import net.havenkeys.android.data.RustVaultRepository
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
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
}
