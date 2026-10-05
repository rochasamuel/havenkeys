package net.havenkeys.android.data

import uniffi.havenkeys_mobile.DeviceInfo
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.PairingRequestView
import uniffi.havenkeys_mobile.Status

interface AccountRepository {
    suspend fun scanKit(frame: LumaFrame): Outcome<KitPreview?>
    fun forgetKit()
    suspend fun signInWithKit(password: String): Outcome<Status>
    suspend fun signIn(server: String, email: String, password: String, secretKey: String): Outcome<Status>
    suspend fun activate(invite: String, password: String): Outcome<Status>
    /** [fresh]: the sync must start after this call, not join one already running (see [Coalescer]). */
    suspend fun syncNow(fresh: Boolean = false): Outcome<Unit>
    suspend fun syncIfDue(): Outcome<Unit>
    suspend fun devices(): Outcome<List<DeviceInfo>>
    suspend fun revoke(id: String): Outcome<Unit>
    suspend fun signOut(): Outcome<Unit>
    suspend fun removeDevice(confirmation: String): Outcome<Unit>

    /** Rust checks the email and the master password; the password goes straight to Rust. */
    suspend fun deleteAccount(confirmation: String, masterPassword: String): Outcome<Unit>

    /** A `havenkeys://pair/v1` link in [frame], or null; the frame's pixels are wiped after. */
    suspend fun scanPairing(frame: LumaFrame): Outcome<String?>
    suspend fun pairingRequest(link: String): Outcome<PairingRequestView>

    /** Call only after the user passed the biometric check. */
    suspend fun approvePairing(link: String): Outcome<Unit>
    suspend fun denyPairing(link: String): Outcome<Unit>
}

class RustAccountRepository(private val vault: MobileVault) : AccountRepository {
    override suspend fun scanKit(frame: LumaFrame) = rust { vault.scanKit(frame) }
    override suspend fun scanPairing(frame: LumaFrame) = rust {
        try {
            vault.scanPairing(frame)
        } finally {
            frame.bytes.fill(0)
        }
    }
    override suspend fun pairingRequest(link: String) = rust { vault.pairingRequest(link) }
    override suspend fun approvePairing(link: String) = rust { vault.approvePairing(link) }
    override suspend fun denyPairing(link: String) = rust { vault.denyPairing(link) }
    override fun forgetKit() = vault.forgetKit()
    override suspend fun signInWithKit(password: String) = rust { vault.signInWithKit(password) }
    override suspend fun signIn(server: String, email: String, password: String, secretKey: String) =
        rust { vault.signIn(server, email, password, secretKey) }
    override suspend fun activate(invite: String, password: String) = rust { vault.activate(invite, password) }
    private val syncing = Coalescer<Unit>()

    /** One owner of the sync: concurrent requests share the run in flight. */
    override suspend fun syncNow(fresh: Boolean) = syncing.run(fresh) { rust { vault.syncNow() } }
    override suspend fun syncIfDue() = rust { vault.syncIfDue() }
    override suspend fun devices() = rust { vault.devices() }
    override suspend fun revoke(id: String) = rust { vault.revokeDevice(id) }
    override suspend fun signOut() = rust { vault.signOut() }
    override suspend fun removeDevice(confirmation: String) = rust { vault.removeDevice(confirmation) }
    override suspend fun deleteAccount(confirmation: String, masterPassword: String) =
        rust { vault.deleteAccount(confirmation, masterPassword) }
}
