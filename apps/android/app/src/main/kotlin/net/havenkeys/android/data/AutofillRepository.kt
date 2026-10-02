package net.havenkeys.android.data

import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.BoundFill
import uniffi.havenkeys_mobile.CardChoices
import uniffi.havenkeys_mobile.CardFrameRoles
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.FrameFacts
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue
import uniffi.havenkeys_mobile.MobileVault
import uniffi.havenkeys_mobile.SaveCard
import uniffi.havenkeys_mobile.SaveLogin
import uniffi.havenkeys_mobile.SaveResult
import uniffi.havenkeys_mobile.TargetFacts
import uniffi.havenkeys_mobile.TargetKind

interface AutofillRepository {
    suspend fun targetKind(target: TargetFacts): Outcome<TargetKind>
    suspend fun confirmBeforeFilling(): Boolean
    suspend fun matches(target: TargetFacts): Outcome<List<AutofillMatch>>
    suspend fun fill(id: String, target: TargetFacts): Outcome<FillValues>
    suspend fun totp(id: String, target: TargetFacts): Outcome<String>
    suspend fun search(query: String): Outcome<List<AutofillMatch>>
    suspend fun bindAndFill(id: String, target: TargetFacts): Outcome<BoundFill>
    suspend fun save(target: TargetFacts, login: SaveLogin): Outcome<SaveResult>
    suspend fun cards(target: TargetFacts, frames: List<FrameFacts>): Outcome<CardChoices>
    suspend fun cardValues(
        id: String,
        target: TargetFacts,
        frames: List<CardFrameRoles>,
    ): Outcome<List<List<CardValue>>>
    suspend fun identity(target: TargetFacts, frame: FrameFacts): Outcome<IdentityChoice?>
    suspend fun identityValues(
        target: TargetFacts,
        frame: FrameFacts,
        roles: List<IdentityRole>,
        documents: Boolean,
    ): Outcome<List<IdentityValue>>
    suspend fun saveCard(target: TargetFacts, frame: FrameFacts, card: SaveCard): Outcome<SaveResult>
}

class RustAutofillRepository(private val vault: MobileVault) : AutofillRepository {
    override suspend fun targetKind(target: TargetFacts) = rust { vault.autofillTargetKind(target) }

    // A failed read asks for confirmation: the safe side.
    override suspend fun confirmBeforeFilling() =
        (rust { vault.confirmBeforeFilling() } as? Outcome.Ok)?.value ?: true
    override suspend fun matches(target: TargetFacts) = rust { vault.autofillMatches(target) }
    override suspend fun fill(id: String, target: TargetFacts) = rust { vault.autofillFill(id, target) }
    override suspend fun totp(id: String, target: TargetFacts) = rust { vault.autofillTotp(id, target) }
    override suspend fun search(query: String) = rust { vault.autofillSearch(query) }
    override suspend fun bindAndFill(id: String, target: TargetFacts) = rust { vault.autofillBindAndFill(id, target) }
    override suspend fun save(target: TargetFacts, login: SaveLogin) = rust { vault.autofillSave(target, login) }

    override suspend fun cards(target: TargetFacts, frames: List<FrameFacts>) =
        rust { vault.autofillCards(target, frames) }
    override suspend fun cardValues(id: String, target: TargetFacts, frames: List<CardFrameRoles>) =
        rust { vault.autofillCardValues(id, target, frames) }
    override suspend fun identity(target: TargetFacts, frame: FrameFacts) =
        rust { vault.autofillIdentity(target, frame) }
    override suspend fun identityValues(
        target: TargetFacts,
        frame: FrameFacts,
        roles: List<IdentityRole>,
        documents: Boolean,
    ) = rust { vault.autofillIdentityValues(target, frame, roles, documents) }
    override suspend fun saveCard(target: TargetFacts, frame: FrameFacts, card: SaveCard) =
        rust { vault.autofillSaveCard(target, frame, card) }
}
