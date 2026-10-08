package net.havenkeys.android.data

import uniffi.havenkeys_mobile.Generated
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.HealthKind
import uniffi.havenkeys_mobile.HealthView
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.Status
import uniffi.havenkeys_mobile.TotpNow
import uniffi.havenkeys_mobile.TrashSummary

@Suppress("TooManyFunctions") // the one seam to the Rust vault; activity calls extend it
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
    /** An `otpauth://totp` link in [frame], or null; the frame's pixels are wiped after. */
    suspend fun scanTotp(frame: LumaFrame): Outcome<String?>
    suspend fun editable(id: String): Outcome<ItemEdit>
    suspend fun template(kind: ItemKind): Outcome<ItemEdit>
    /** Online only; the new item's id. */
    suspend fun create(draft: ItemDraft): Outcome<String>
    suspend fun update(id: String, draft: ItemDraft): Outcome<Unit>
    /** Delete moves the item to the Trash; `false` when it was deleted for good (its details did not open). */
    suspend fun trash(id: String): Outcome<Boolean>
    suspend fun restore(id: String): Outcome<Unit>
    suspend fun purge(id: String): Outcome<Unit>
    suspend fun emptyTrash(): Outcome<Int>
    /** Overviews only, newest first. */
    suspend fun listTrash(): Outcome<List<TrashSummary>>
    suspend fun recordUse(id: String): Outcome<Unit>
    suspend fun frequentlyUsed(n: Int): Outcome<List<ItemSummary>>
    suspend fun recentlyCreated(n: Int): Outcome<List<ItemSummary>>
    suspend fun recentSearches(): Outcome<List<String>>
    suspend fun recordSearch(query: String): Outcome<Unit>
    suspend fun clearRecentSearches(): Outcome<Unit>
    /** Vault health: ids and check kinds only. Slow on a large vault; never counts as user activity. */
    suspend fun health(): Outcome<HealthView>
    /** Online only; replaces the login's whole list of dismissed checks. */
    suspend fun setHealthIgnored(id: String, kinds: List<HealthKind>): Outcome<Unit>
    /** Rust's help link for the check, from the login's own websites; opened unchanged. */
    suspend fun healthHelpUrl(id: String, kind: HealthKind): Outcome<String>
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
    override suspend fun scanTotp(frame: LumaFrame) = rust {
        try {
            vault.scanTotp(frame)
        } finally {
            frame.bytes.fill(0)
        }
    }
    override suspend fun editable(id: String) = rust { vault.itemEdit(id) }
    override suspend fun template(kind: ItemKind) = rust { vault.itemTemplate(kind) }
    override suspend fun create(draft: ItemDraft) = rust { vault.createItem(draft) }
    override suspend fun update(id: String, draft: ItemDraft) = rust { vault.updateItem(id, draft) }
    override suspend fun trash(id: String) = rust { vault.trashItem(id) }
    override suspend fun restore(id: String) = rust { vault.restoreItem(id) }
    override suspend fun purge(id: String) = rust { vault.purgeItem(id) }
    override suspend fun emptyTrash() = rust { vault.emptyTrash().toInt() }
    override suspend fun listTrash() = rust { vault.listTrash() }
    override suspend fun recordUse(id: String) = rust { vault.recordUse(id) }
    override suspend fun frequentlyUsed(n: Int) = rust { vault.frequentlyUsed(n.toUInt()) }
    override suspend fun recentlyCreated(n: Int) = rust { vault.recentlyCreated(n.toUInt()) }
    override suspend fun recentSearches() = rust { vault.recentSearches() }
    override suspend fun recordSearch(query: String) = rust { vault.recordSearch(query) }
    override suspend fun clearRecentSearches() = rust { vault.clearRecentSearches() }
    override suspend fun health() = rust { vault.healthReport() }
    override suspend fun setHealthIgnored(id: String, kinds: List<HealthKind>) =
        rust { vault.setHealthIgnored(id, kinds) }
    override suspend fun healthHelpUrl(id: String, kind: HealthKind) = rust { vault.healthHelpUrl(id, kind) }
}
