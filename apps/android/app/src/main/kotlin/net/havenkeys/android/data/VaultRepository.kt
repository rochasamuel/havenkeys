package net.havenkeys.android.data

import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.Status
import uniffi.havenkeys_mobile.TotpNow

interface VaultRepository {
    suspend fun status(): Outcome<Status>
    /** [secretKey] only when the device lacks it (`Status.needsSecretKey`). */
    suspend fun unlockPassword(password: String, secretKey: String? = null): Outcome<Status>
    suspend fun createUnlockBundle(password: String, bootCount: Long): Outcome<ByteArray>
    suspend fun unlockWithBundle(bundle: ByteArray, bootCount: Long): Outcome<Status>
    fun lock()
    fun touch()
    fun tick()
    fun screenTurnedOff()
    suspend fun list(): Outcome<List<ItemSummary>>
    suspend fun search(query: String): Outcome<List<ItemSummary>>
    suspend fun view(id: String): Outcome<ItemView>
    suspend fun reveal(id: String, key: String): Outcome<String>
    suspend fun totp(id: String): Outcome<TotpNow>
    suspend fun generate(options: GeneratorOptions): Outcome<Generated>
}

class RustVaultRepository(private val vault: MobileVault) : VaultRepository {
    override suspend fun status() = rust { vault.status() }
    override suspend fun unlockPassword(password: String, secretKey: String?) =
        rust { vault.unlockPassword(password, secretKey) }
    override suspend fun createUnlockBundle(password: String, bootCount: Long) =
        rust { vault.createUnlockBundle(password, bootCount) }
    override suspend fun unlockWithBundle(bundle: ByteArray, bootCount: Long) = rust {
        try {
            vault.unlockWithBundle(bundle, bootCount)
        } finally {
            bundle.fill(0)
        }
    }
    override fun lock() = vault.lock()
    override fun touch() = vault.touch()
    override fun tick() = vault.tick()
    override fun screenTurnedOff() = vault.screenTurnedOff()
    override suspend fun list() = rust { vault.listItems() }
    override suspend fun search(query: String) = rust { vault.search(query) }
    override suspend fun view(id: String) = rust { vault.itemView(id) }
    override suspend fun reveal(id: String, key: String) = rust { vault.reveal(id, key) }
    override suspend fun totp(id: String) = rust { vault.totp(id) }
    override suspend fun generate(options: GeneratorOptions) = rust { vault.generatePassword(options) }
}
