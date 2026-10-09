package net.havenkeys.android.data

import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.PasskeyCreatePlan
import uniffi.havenkeys_mobile.PasskeyOffer

/** Credential Manager (spec §8). Rust decides every answer; this only carries the caller's facts. */
interface CredentialRepository {
    suspend fun passkeyOffers(caller: CredentialCaller, requestJson: String): Outcome<List<PasskeyOffer>>
    suspend fun passkeySignIn(
        caller: CredentialCaller,
        requestJson: String,
        clientDataHash: ByteArray?,
        itemId: String,
        credentialId: ByteArray,
    ): Outcome<String>
    suspend fun passkeyCreatePlan(caller: CredentialCaller, requestJson: String): Outcome<PasskeyCreatePlan>
    suspend fun passkeyCreate(caller: CredentialCaller, requestJson: String, itemId: String?): Outcome<String>
    suspend fun passwordOffers(caller: CredentialCaller): Outcome<List<AutofillMatch>>
    suspend fun password(caller: CredentialCaller, itemId: String): Outcome<FillValues>

    /** Trial ended: no passkey can be saved. A failed read is not frozen; Rust refuses anyway. */
    suspend fun frozen(): Boolean
}

class RustCredentialRepository(private val vault: MobileVault) : CredentialRepository {
    override suspend fun passkeyOffers(caller: CredentialCaller, requestJson: String) =
        rust { vault.passkeyOffers(caller, requestJson) }
    override suspend fun passkeySignIn(
        caller: CredentialCaller,
        requestJson: String,
        clientDataHash: ByteArray?,
        itemId: String,
        credentialId: ByteArray,
    ) = rust { vault.passkeySignIn(caller, requestJson, clientDataHash, itemId, credentialId) }
    override suspend fun passkeyCreatePlan(caller: CredentialCaller, requestJson: String) =
        rust { vault.passkeyCreatePlan(caller, requestJson) }
    override suspend fun passkeyCreate(caller: CredentialCaller, requestJson: String, itemId: String?) =
        rust { vault.passkeyCreate(caller, requestJson, itemId) }
    override suspend fun passwordOffers(caller: CredentialCaller) = rust { vault.credentialPasswordOffers(caller) }
    override suspend fun password(caller: CredentialCaller, itemId: String) =
        rust { vault.credentialPassword(caller, itemId) }
    override suspend fun frozen() = (rust { vault.frozen() } as? Outcome.Ok)?.value ?: false
}
