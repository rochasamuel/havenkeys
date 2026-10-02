package net.havenkeys.android.autofill

import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.CardChoices
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.TargetFacts

class WalletConfirmTest {
    private val target = TargetFacts("com.shop.android", listOf(ByteArray(32)), null, null)
    private val form = CardForm(listOf(CardFrame(null, null, listOf(CardField(0, CardKind.NUMBER, false)))))
    private val visa = CardChoice("c1", "Visa", "visa", "1111", "2033-04")

    private fun repoOffering(frames: List<Boolean>) = FakeAutofillRepository().apply {
        cardChoices = Outcome.Ok(CardChoices(false, listOf(visa), frames))
    }

    @Test
    fun preparingACardConfirmationAsksRustForNoValue() = runTest {
        val repo = repoOffering(listOf(true))
        assertNotNull(CardConfirmation.prepare(repo, form, target, "c1"))
        assertTrue(repo.calls.none { it.startsWith("cardValues") })
    }

    @Test
    fun aCardNotOfferedToThisFormIsRefused() = runTest {
        val repo = repoOffering(listOf(true))
        assertNull(CardConfirmation.prepare(repo, form, target, "someone-elses-id"))
        repo.cardChoices = Outcome.Ok(CardChoices(false, listOf(visa), listOf(false)))
        assertNull(CardConfirmation.prepare(repo, form, target, "c1"))
        repo.cardChoices = Outcome.Failed("locked")
        assertNull(CardConfirmation.prepare(repo, form, target, "c1"))
    }

    @Test
    fun documentsAreAskedForOnlyWhenChosen() = runTest {
        val identityForm = IdentityForm(
            listOf(IdentityField(0, IdentityRole.FIRST_NAME), IdentityField(1, IdentityRole.CPF)),
            null,
            null,
        )
        val repo = FakeAutofillRepository().apply {
            identityChoice = Outcome.Ok(
                IdentityChoice("Samuel", null, listOf(IdentityRole.FIRST_NAME, IdentityRole.CPF)),
            )
        }
        val confirmation = IdentityConfirmation.prepare(repo, identityForm, target)!!
        assertTrue(repo.calls.none { it == "identityValues" })
        confirmation.values(repo, target, withDocuments = false)
        confirmation.values(repo, target, withDocuments = true)
        assertEquals(
            listOf(
                listOf(IdentityRole.FIRST_NAME) to false,
                listOf(IdentityRole.FIRST_NAME, IdentityRole.CPF) to true,
            ),
            repo.identityAsks,
        )
    }
}
