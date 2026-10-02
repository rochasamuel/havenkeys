package net.havenkeys.android.credentials

import net.havenkeys.android.data.CredentialRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.PasskeyOffer

/** One option of a Begin request, by its position in the request. */
sealed interface Asked {
    data class Passkey(val index: Int, val requestJson: String) : Asked
    data class Password(val index: Int) : Asked
}

sealed interface CredentialOffer {
    data object Unlock : CredentialOffer
    data class Passkey(val index: Int, val offer: PasskeyOffer) : CredentialOffer
    data class Password(val index: Int, val match: AutofillMatch) : CredentialOffer
}

/**
 * What Android's sheet lists for a Begin request. Every entry comes from
 * Rust, which re-checks the caller on each call; nothing from the vault is
 * read while it is locked.
 */
object CredentialPlanner {
    const val MAX_PASSWORDS = 5

    suspend fun plan(
        asked: List<Asked>,
        caller: CredentialCaller,
        unlocked: Boolean,
        repo: CredentialRepository,
    ): List<CredentialOffer> = when {
        asked.isEmpty() -> emptyList()
        !unlocked -> listOf(CredentialOffer.Unlock)
        else -> list(asked, caller, repo)
    }

    private suspend fun list(
        asked: List<Asked>,
        caller: CredentialCaller,
        repo: CredentialRepository,
    ): List<CredentialOffer> {
        val offers = mutableListOf<CredentialOffer>()
        var passwordsListed = false
        for (option in asked) {
            val found = when (option) {
                is Asked.Passkey -> passkeys(option, caller, repo)
                is Asked.Password -> if (passwordsListed) {
                    Outcome.Ok(emptyList())
                } else {
                    passwordsListed = true
                    passwords(option, caller, repo)
                }
            }
            // Rust applied an overdue auto-lock on this very request.
            if (found is Outcome.Failed && found.code == "locked") return listOf(CredentialOffer.Unlock)
            if (found is Outcome.Ok) offers += found.value
        }
        return offers
    }

    private suspend fun passkeys(
        option: Asked.Passkey,
        caller: CredentialCaller,
        repo: CredentialRepository,
    ): Outcome<List<CredentialOffer>> = when (val r = repo.passkeyOffers(caller, option.requestJson)) {
        is Outcome.Ok -> Outcome.Ok(r.value.map { CredentialOffer.Passkey(option.index, it) })
        is Outcome.Failed -> r
    }

    private suspend fun passwords(
        option: Asked.Password,
        caller: CredentialCaller,
        repo: CredentialRepository,
    ): Outcome<List<CredentialOffer>> = when (val r = repo.passwordOffers(caller)) {
        is Outcome.Ok -> Outcome.Ok(r.value.take(MAX_PASSWORDS).map { CredentialOffer.Password(option.index, it) })
        is Outcome.Failed -> r
    }
}
