package net.havenkeys.android.data

import uniffi.havenkeys_mobile.MobileSettings
import uniffi.havenkeys_mobile.MobileVault

interface SettingsRepository {
    suspend fun get(): Outcome<MobileSettings>
    suspend fun update(settings: MobileSettings): Outcome<Unit>
}

class RustSettingsRepository(private val vault: MobileVault) : SettingsRepository {
    override suspend fun get() = rust { vault.settings() }
    override suspend fun update(settings: MobileSettings) = rust { vault.updateSettings(settings) }
}
