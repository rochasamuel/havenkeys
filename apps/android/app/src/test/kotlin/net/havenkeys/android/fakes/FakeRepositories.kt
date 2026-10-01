package net.havenkeys.android.fakes

import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.BoundFill
import uniffi.havenkeys_mobile.DeviceInfo
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LockState
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.MobileSettings
import uniffi.havenkeys_mobile.Status
import uniffi.havenkeys_mobile.TargetFacts
import uniffi.havenkeys_mobile.TargetKind
import uniffi.havenkeys_mobile.TotpNow

fun status(state: LockState = LockState.UNLOCKED, exists: Boolean = true) =
    Status(state, exists, false, false, "user@example.com", "https://vault.example.com", null, 0u)

fun settings() = MobileSettings(
    autoLockMinutes = 15u,
    clipboardClearSeconds = 30u,
    lockOnScreenOff = true,
    confirmBeforeFilling = false,
    assetLinks = true,
)

class FakeVaultRepository : VaultRepository {
    // Qualified: the member status() would shadow the helper here.
    var nextStatus: Outcome<Status> = Outcome.Ok(net.havenkeys.android.fakes.status())
    var items: Outcome<List<ItemSummary>> = Outcome.Ok(emptyList())
    var view: Outcome<ItemView> = Outcome.Failed("not_found")
    var revealed: Outcome<String> = Outcome.Failed("not_found")
    var totpNow: Outcome<TotpNow> = Outcome.Failed("not_found")
    var bundle: Outcome<ByteArray> = Outcome.Failed("internal")
    val calls = mutableListOf<String>()

    override suspend fun status() = nextStatus

    var lastSecretKey: String? = null

    override suspend fun unlockPassword(password: String, secretKey: String?): Outcome<Status> {
        calls += "unlockPassword"
        lastSecretKey = secretKey
        return nextStatus
    }

    override suspend fun createUnlockBundle(password: String, bootCount: Long): Outcome<ByteArray> {
        calls += "createUnlockBundle"
        return bundle
    }

    override suspend fun unlockWithBundle(bundle: ByteArray, bootCount: Long): Outcome<Status> {
        calls += "unlockWithBundle"
        return nextStatus
    }

    override fun lock() {
        calls += "lock"
    }

    override fun touch() {
        calls += "touch"
    }

    override fun tick() {
        calls += "tick"
    }

    override fun screenTurnedOff() {
        calls += "screenTurnedOff"
    }

    override suspend fun list() = items

    override suspend fun search(query: String): Outcome<List<ItemSummary>> {
        calls += "search"
        return items
    }

    override suspend fun view(id: String) = view

    override suspend fun reveal(id: String, key: String): Outcome<String> {
        calls += "reveal:$key"
        return revealed
    }

    override suspend fun totp(id: String): Outcome<TotpNow> {
        calls += "totp"
        return totpNow
    }

    override suspend fun generate(options: GeneratorOptions): Outcome<Generated> =
        Outcome.Ok(Generated("x".repeat(options.length.toInt()), 100.0))
}

class FakeAccountRepository : AccountRepository {
    var kit: Outcome<KitPreview?> = Outcome.Ok(null)
    var nextStatus: Outcome<Status> = Outcome.Ok(status())
    var sync: Outcome<Unit> = Outcome.Ok(Unit)
    var deviceList: Outcome<List<DeviceInfo>> = Outcome.Ok(emptyList())
    var done: Outcome<Unit> = Outcome.Ok(Unit)
    val calls = mutableListOf<String>()

    override suspend fun scanKit(frame: LumaFrame): Outcome<KitPreview?> {
        calls += "scanKit"
        return kit
    }

    override fun forgetKit() {
        calls += "forgetKit"
    }

    override suspend fun signInWithKit(password: String): Outcome<Status> {
        calls += "signInWithKit"
        return nextStatus
    }

    override suspend fun signIn(server: String, email: String, password: String, secretKey: String): Outcome<Status> {
        calls += "signIn"
        return nextStatus
    }

    override suspend fun activate(invite: String, password: String): Outcome<Status> {
        calls += "activate"
        return nextStatus
    }

    override suspend fun syncNow(): Outcome<Unit> {
        calls += "syncNow"
        return sync
    }

    override suspend fun syncIfDue(): Outcome<Unit> {
        calls += "syncIfDue"
        return sync
    }

    override suspend fun devices() = deviceList

    override suspend fun revoke(id: String): Outcome<Unit> {
        calls += "revoke:$id"
        return done
    }

    override suspend fun signOut(): Outcome<Unit> {
        calls += "signOut"
        return done
    }

    override suspend fun removeDevice(confirmation: String): Outcome<Unit> {
        calls += "removeDevice"
        return done
    }
}

class FakeSettingsRepository : SettingsRepository {
    var current: Outcome<MobileSettings> = Outcome.Ok(settings())
    var updateResult: Outcome<Unit> = Outcome.Ok(Unit)
    val updates = mutableListOf<MobileSettings>()

    override suspend fun get() = current

    override suspend fun update(settings: MobileSettings): Outcome<Unit> {
        updates += settings
        if (updateResult is Outcome.Ok) current = Outcome.Ok(settings)
        return updateResult
    }
}

class FakeAutofillRepository : AutofillRepository {
    var kind: Outcome<TargetKind> = Outcome.Ok(TargetKind.BROWSER)
    var confirm = false
    var matchList: Outcome<List<AutofillMatch>> = Outcome.Ok(emptyList())
    var values: Outcome<FillValues> = Outcome.Failed("not_found")
    var code: Outcome<String> = Outcome.Failed("not_found")
    var bound: Outcome<BoundFill> = Outcome.Failed("not_found")
    val calls = mutableListOf<String>()

    override suspend fun targetKind(target: TargetFacts) = kind

    override suspend fun confirmBeforeFilling() = confirm

    override suspend fun matches(target: TargetFacts): Outcome<List<AutofillMatch>> {
        calls += "matches"
        return matchList
    }

    override suspend fun fill(id: String, target: TargetFacts): Outcome<FillValues> {
        calls += "fill:$id"
        return values
    }

    override suspend fun totp(id: String, target: TargetFacts): Outcome<String> {
        calls += "totp:$id"
        return code
    }

    override suspend fun search(query: String): Outcome<List<AutofillMatch>> {
        calls += "search"
        return matchList
    }

    override suspend fun bindAndFill(id: String, target: TargetFacts): Outcome<BoundFill> {
        calls += "bindAndFill:$id"
        return bound
    }
}
