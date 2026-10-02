package net.havenkeys.android.credentials

import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeCredentialRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.PasskeyOffer

class CredentialPlannerTest {
    private val caller = CredentialCaller("com.android.chrome", listOf(ByteArray(32)), "https://github.com")
    private val passkey = PasskeyOffer("id1", byteArrayOf(1, 2), "GitHub", "octo")
    private val login = AutofillMatch("id2", "GitHub", "octo", false)

    @Test
    fun nothingAskedIsNothingOffered() = runTest {
        assertTrue(CredentialPlanner.plan(emptyList(), caller, unlocked = false, FakeCredentialRepository()).isEmpty())
    }

    @Test
    fun aLockedVaultOffersOnlyUnlockAndReadsNothing() = runTest {
        val repo = FakeCredentialRepository()
        val offers = CredentialPlanner.plan(listOf(Asked.Passkey(0, "{}")), caller, unlocked = false, repo)
        assertEquals(listOf(CredentialOffer.Unlock), offers)
        assertTrue(repo.calls.isEmpty())
    }

    @Test
    fun passkeysAndPasswordsKeepTheIndexOfTheirOption() = runTest {
        val repo = FakeCredentialRepository().apply {
            passkeys = Outcome.Ok(listOf(passkey))
            passwords = Outcome.Ok(listOf(login))
        }
        val offers = CredentialPlanner.plan(listOf(Asked.Password(0), Asked.Passkey(1, "{}")), caller, true, repo)
        assertEquals(2, offers.size)
        assertEquals(0, (offers[0] as CredentialOffer.Password).index)
        assertEquals("id2", (offers[0] as CredentialOffer.Password).match.id)
        assertEquals(1, (offers[1] as CredentialOffer.Passkey).index)
        assertEquals("id1", (offers[1] as CredentialOffer.Passkey).offer.itemId)
    }

    @Test
    fun passwordsAreAskedOnceAndCapped() = runTest {
        val many = (1..10).map { login.copy(id = "id$it") }
        val repo = FakeCredentialRepository().apply { passwords = Outcome.Ok(many) }
        val offers = CredentialPlanner.plan(listOf(Asked.Password(0), Asked.Password(1)), caller, true, repo)
        assertEquals(CredentialPlanner.MAX_PASSWORDS, offers.size)
        assertEquals(1, repo.calls.count { it == "passwordOffers" })
    }

    @Test
    fun aRefusedOptionOffersNothingForIt() = runTest {
        val repo = FakeCredentialRepository().apply {
            passkeys = Outcome.Failed("denied")
            passwords = Outcome.Ok(listOf(login))
        }
        val offers = CredentialPlanner.plan(listOf(Asked.Passkey(0, "{}"), Asked.Password(1)), caller, true, repo)
        assertEquals(listOf("id2"), offers.map { (it as CredentialOffer.Password).match.id })
    }

    @Test
    fun aLockDuringListingOffersOnlyUnlock() = runTest {
        val repo = FakeCredentialRepository().apply {
            passwords = Outcome.Ok(listOf(login))
            passkeys = Outcome.Failed("locked")
        }
        val offers = CredentialPlanner.plan(listOf(Asked.Password(0), Asked.Passkey(1, "{}")), caller, true, repo)
        assertEquals(listOf(CredentialOffer.Unlock), offers)
    }
}
