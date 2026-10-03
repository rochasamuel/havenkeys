package net.havenkeys.android.data

import uniffi.havenkeys_mobile.DeviceInfo
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.Status

interface AccountRepository {
    suspend fun scanKit(frame: LumaFrame): Outcome<KitPreview?>
    fun forgetKit()
    suspend fun signInWithKit(password: String): Outcome<Status>
    suspend fun signIn(server: String, email: String, password: String, secretKey: String): Outcome<Status>
    suspend fun activate(invite: String, password: String): Outcome<Status>
    suspend fun syncNow(): Outcome<Unit>
    suspend fun syncIfDue(): Outcome<Unit>
    suspend fun devices(): Outcome<List<DeviceInfo>>
    suspend fun revoke(id: String): Outcome<Unit>
    suspend fun signOut(): Outcome<Unit>
    suspend fun removeDevice(confirmation: String): Outcome<Unit>
}

class RustAccountRepository(private val vault: MobileVault) : AccountRepository {
    override suspend fun scanKit(frame: LumaFrame) = rust { vault.scanKit(frame) }
    override fun forgetKit() = vault.forgetKit()
    override suspend fun signInWithKit(password: String) = rust { vault.signInWithKit(password) }
    override suspend fun signIn(server: String, email: String, password: String, secretKey: String) =
        rust { vault.signIn(server, email, password, secretKey) }
    override suspend fun activate(invite: String, password: String) = rust { vault.activate(invite, password) }
    private val syncing = Coalescer<Unit>()

    /** One owner of the sync: concurrent requests share the run in flight. */
    override suspend fun syncNow() = syncing.run { rust { vault.syncNow() } }
    override suspend fun syncIfDue() = rust { vault.syncIfDue() }
    override suspend fun devices() = rust { vault.devices() }
    override suspend fun revoke(id: String) = rust { vault.revokeDevice(id) }
    override suspend fun signOut() = rust { vault.signOut() }
    override suspend fun removeDevice(confirmation: String) = rust { vault.removeDevice(confirmation) }
}
