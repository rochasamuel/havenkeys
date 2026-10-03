package net.havenkeys.android.fakes

import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.AutofillRepository
import net.havenkeys.android.data.CredentialRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.BoundFill
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.DeviceInfo
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LockState
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.MobileSettings
import uniffi.havenkeys_mobile.PasskeyCreatePlan
import uniffi.havenkeys_mobile.PasskeyOffer
import uniffi.havenkeys_mobile.CardChoices
import uniffi.havenkeys_mobile.CardFrameRoles
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.FrameFacts
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue
import uniffi.havenkeys_mobile.SaveCard
import uniffi.havenkeys_mobile.SaveLogin
import uniffi.havenkeys_mobile.SaveResult
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
    var lastPassword: String? = null

    override suspend fun unlockPassword(password: String, secretKey: String?): Outcome<Status> {
        calls += "unlockPassword"
        lastPassword = password
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

    var generatedWith: GeneratorOptions? = null
    var generated: Outcome<Generated>? = null

    override suspend fun generate(options: GeneratorOptions): Outcome<Generated> {
        generatedWith = options
        return generated ?: Outcome.Ok(Generated("x".repeat(options.length.toInt()), 100.0))
    }

    var edit: Outcome<ItemEdit> = Outcome.Failed("not_found")
    var created: Outcome<String> = Outcome.Ok("new-id")
    var updated: Outcome<Unit> = Outcome.Ok(Unit)
    var deleted: Outcome<Unit> = Outcome.Ok(Unit)
    /** Drafts handed over, for assertions; a test fake only. */
    val drafts = mutableListOf<ItemDraft>()

    override suspend fun editable(id: String): Outcome<ItemEdit> {
        calls += "editable:$id"
        return edit
    }

    override suspend fun template(kind: ItemKind): Outcome<ItemEdit> {
        calls += "template:$kind"
        return edit
    }

    override suspend fun create(draft: ItemDraft): Outcome<String> {
        calls += "create"
        drafts += draft
        return created
    }

    override suspend fun update(id: String, draft: ItemDraft): Outcome<Unit> {
        calls += "update:$id"
        drafts += draft
        return updated
    }

    override suspend fun delete(id: String): Outcome<Unit> {
        calls += "delete:$id"
        return deleted
    }

    val usesRecorded = mutableListOf<String>()
    var frequent: Outcome<List<ItemSummary>> = Outcome.Ok(emptyList())
    var recent: Outcome<List<ItemSummary>> = Outcome.Ok(emptyList())
    val searches = mutableListOf<String>()

    override suspend fun recordUse(id: String): Outcome<Unit> {
        usesRecorded += id
        return Outcome.Ok(Unit)
    }

    override suspend fun frequentlyUsed(n: Int): Outcome<List<ItemSummary>> {
        calls += "frequent:$n"
        return frequent
    }

    override suspend fun recentlyCreated(n: Int): Outcome<List<ItemSummary>> {
        calls += "recent:$n"
        return recent
    }

    override suspend fun recentSearches(): Outcome<List<String>> = Outcome.Ok(searches.toList())

    override suspend fun recordSearch(query: String): Outcome<Unit> {
        searches.removeAll { it.equals(query.trim(), ignoreCase = true) }
        if (query.isNotBlank()) searches.add(0, query.trim())
        return Outcome.Ok(Unit)
    }

    override suspend fun clearRecentSearches(): Outcome<Unit> {
        searches.clear()
        return Outcome.Ok(Unit)
    }
}

class FakeAccountRepository : AccountRepository {
    var kit: Outcome<KitPreview?> = Outcome.Ok(null)
    var nextStatus: Outcome<Status> = Outcome.Ok(status())
    var sync: Outcome<Unit> = Outcome.Ok(Unit)
    var deviceList: Outcome<List<DeviceInfo>> = Outcome.Ok(emptyList())
    var done: Outcome<Unit> = Outcome.Ok(Unit)
    val calls = mutableListOf<String>()
    val freshCalls = mutableListOf<Boolean>()

    /** When set, syncNow suspends until it completes. */
    var syncGate: kotlinx.coroutines.CompletableDeferred<Unit>? = null

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

    override suspend fun syncNow(fresh: Boolean): Outcome<Unit> {
        calls += "syncNow"
        freshCalls += fresh
        syncGate?.await()
        return sync
    }

    override suspend fun syncIfDue(): Outcome<Unit> {
        calls += "syncIfDue"
        return sync
    }

    override suspend fun devices(): Outcome<List<DeviceInfo>> {
        calls += "devices"
        return deviceList
    }

    override suspend fun revoke(id: String): Outcome<Unit> {
        calls += "revoke:$id"
        return done
    }

    override suspend fun signOut(): Outcome<Unit> {
        calls += "signOut"
        return done
    }

    override suspend fun removeDevice(confirmation: String): Outcome<Unit> {
        calls += "removeDevice:$confirmation"
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
    val usesRecorded = mutableListOf<String>()

    override suspend fun recordUse(id: String): Outcome<Unit> {
        usesRecorded += id
        return Outcome.Ok(Unit)
    }

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

    var saved: Outcome<SaveResult> = Outcome.Ok(SaveResult.ADDED)
    val saves = mutableListOf<Pair<TargetFacts, SaveLogin>>()

    override suspend fun save(target: TargetFacts, login: SaveLogin): Outcome<SaveResult> {
        calls += "save"
        saves += target to login
        return saved
    }

    var cardChoices: Outcome<CardChoices> = Outcome.Ok(CardChoices(false, emptyList(), emptyList()))
    var cardValueList: Outcome<List<List<CardValue>>> = Outcome.Failed("not_found")
    var identityChoice: Outcome<IdentityChoice?> = Outcome.Ok(null)
    var identityValueList: Outcome<List<IdentityValue>> = Outcome.Ok(emptyList())
    var savedCard: Outcome<SaveResult> = Outcome.Ok(SaveResult.ADDED)
    val valueFrames = mutableListOf<List<CardFrameRoles>>()
    val identityAsks = mutableListOf<Pair<List<IdentityRole>, Boolean>>()
    val savedCards = mutableListOf<SaveCard>()

    override suspend fun cards(target: TargetFacts, frames: List<FrameFacts>): Outcome<CardChoices> {
        calls += "cards"
        return cardChoices
    }

    override suspend fun cardValues(
        id: String,
        target: TargetFacts,
        frames: List<CardFrameRoles>,
    ): Outcome<List<List<CardValue>>> {
        calls += "cardValues:$id"
        valueFrames += frames
        return cardValueList
    }

    override suspend fun identity(target: TargetFacts, frame: FrameFacts): Outcome<IdentityChoice?> {
        calls += "identity"
        return identityChoice
    }

    override suspend fun identityValues(
        target: TargetFacts,
        frame: FrameFacts,
        roles: List<IdentityRole>,
        documents: Boolean,
    ): Outcome<List<IdentityValue>> {
        calls += "identityValues"
        identityAsks += roles to documents
        return identityValueList
    }

    override suspend fun saveCard(target: TargetFacts, frame: FrameFacts, card: SaveCard): Outcome<SaveResult> {
        calls += "saveCard"
        savedCards += card
        return savedCard
    }
}

class FakeCredentialRepository : CredentialRepository {
    var passkeys: Outcome<List<PasskeyOffer>> = Outcome.Ok(emptyList())
    var passwords: Outcome<List<AutofillMatch>> = Outcome.Ok(emptyList())
    var plan: Outcome<PasskeyCreatePlan> = Outcome.Failed("denied")
    var created: Outcome<String> = Outcome.Failed("denied")
    var signedIn: Outcome<String> = Outcome.Failed("denied")
    var passwordValues: Outcome<FillValues> = Outcome.Failed("denied")
    val calls = mutableListOf<String>()

    override suspend fun passkeyOffers(caller: CredentialCaller, requestJson: String): Outcome<List<PasskeyOffer>> {
        calls += "passkeyOffers"
        return passkeys
    }

    override suspend fun passkeySignIn(
        caller: CredentialCaller,
        requestJson: String,
        clientDataHash: ByteArray?,
        itemId: String,
        credentialId: ByteArray,
    ): Outcome<String> {
        calls += "passkeySignIn:$itemId"
        return signedIn
    }

    override suspend fun passkeyCreatePlan(
        caller: CredentialCaller,
        requestJson: String,
    ): Outcome<PasskeyCreatePlan> {
        calls += "passkeyCreatePlan"
        return plan
    }

    override suspend fun passkeyCreate(
        caller: CredentialCaller,
        requestJson: String,
        itemId: String?,
    ): Outcome<String> {
        calls += "passkeyCreate:${itemId ?: "new"}"
        return created
    }

    override suspend fun passwordOffers(caller: CredentialCaller): Outcome<List<AutofillMatch>> {
        calls += "passwordOffers"
        return passwords
    }

    override suspend fun password(caller: CredentialCaller, itemId: String): Outcome<FillValues> {
        calls += "password:$itemId"
        return passwordValues
    }
}
