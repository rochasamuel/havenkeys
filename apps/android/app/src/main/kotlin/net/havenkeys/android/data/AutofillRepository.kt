package net.havenkeys.android.data

import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.BoundFill
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.MobileVault
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
}
