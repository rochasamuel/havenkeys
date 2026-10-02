package net.havenkeys.android.autofill

import java.time.YearMonth
import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.CardChoice
import uniffi.havenkeys_mobile.CardChoices
import uniffi.havenkeys_mobile.CardRole
import uniffi.havenkeys_mobile.CardValue
import uniffi.havenkeys_mobile.IdentityChoice
import uniffi.havenkeys_mobile.IdentityRole
import uniffi.havenkeys_mobile.IdentityValue
import uniffi.havenkeys_mobile.TargetFacts

class WalletPlannerTest {
    private val target = TargetFacts("com.android.chrome", listOf(ByteArray(32)), "shop.example.com", "https")
    private val today = YearMonth.of(2026, 10)
    private val shop = CardFrame(
        "shop.example.com", "https",
        listOf(CardField(0, CardKind.CARDHOLDER_NAME, false), CardField(1, CardKind.NUMBER, false)),
    )
    private val ads = CardFrame("ads.example.net", "https", listOf(CardField(2, CardKind.NUMBER, false)))
    private val visa = CardChoice("c1", "Visa", "visa", "1111", "2030-01")
    private val old = CardChoice("c0", "Old", "visa", "0004", "2025-01")
    private val number = listOf(listOf(CardValue(CardRole.NUMBER, "4111111111111111")))

    private fun choices(vararg frames: Boolean, cards: List<CardChoice> = listOf(visa), insecure: Boolean = false) =
        Outcome.Ok(CardChoices(insecure, cards, frames.toList()))

    private suspend fun cards(
        repo: FakeAutofillRepository,
        form: CardForm = CardForm(listOf(shop)),
        unlocked: Boolean = true,
        direct: Boolean = true,
    ) = WalletPlanner.plan(Routed.Card(form), target, unlocked, repo, direct, today)

    @Test
    fun lockedAsksToUnlockFirst() = runTest {
        val repo = FakeAutofillRepository()
        assertEquals(WalletPlan.UnlockFirst, cards(repo, unlocked = false))
        assertTrue("nothing is read while locked", repo.calls.isEmpty())
    }

    @Test
    fun aLockDuringTheRequestAsksToUnlock() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = Outcome.Failed("locked") }
        assertEquals(WalletPlan.UnlockFirst, cards(repo))
    }

    @Test
    fun directFillCarriesEachCardsValues() = runTest {
        val repo = FakeAutofillRepository().apply {
            cardChoices = choices(true)
            cardValueList = Outcome.Ok(number)
        }
        val plan = cards(repo) as WalletPlan.Cards
        assertEquals(number, plan.rows.single().values)
        assertTrue(plan.save)
    }

    @Test
    fun rowsOfferedAfterAnUnlockCarryNoValues() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = choices(true) }
        val plan = cards(repo, direct = false) as WalletPlan.Cards
        assertNull(plan.rows.single().values)
        assertTrue(repo.calls.none { it.startsWith("cardValues") })
    }

    @Test
    fun expiredCardsComeLast() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = choices(true, cards = listOf(old, visa)) }
        val plan = cards(repo, direct = false) as WalletPlan.Cards
        assertEquals(listOf("c1", "c0"), plan.rows.map { it.card.id })
        assertEquals(listOf(false, true), plan.rows.map { it.expired })
    }

    @Test
    fun atMostFiveCards() = runTest {
        val many = (1..8).map { visa.copy(id = "c$it") }
        val repo = FakeAutofillRepository().apply { cardChoices = choices(true, cards = many) }
        assertEquals(WalletPlanner.MAX_CARDS, (cards(repo, direct = false) as WalletPlan.Cards).rows.size)
    }

    @Test
    fun aRefusedFrameIsLeftOutOfTheCard() = runTest {
        val repo = FakeAutofillRepository().apply {
            cardChoices = choices(true, false)
            cardValueList = Outcome.Ok(number)
        }
        val plan = cards(repo, form = CardForm(listOf(shop, ads))) as WalletPlan.Cards
        assertEquals(listOf(shop), plan.frames)
        assertEquals(1, repo.valueFrames.single().size)
    }

    @Test
    fun aRefusedFocusedFrameOffersNothing() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = choices(false, true) }
        assertEquals(WalletPlan.Nothing, cards(repo, form = CardForm(listOf(ads, shop))))
    }

    @Test
    fun anHttpPageOffersNothing() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = choices(false, insecure = true) }
        assertEquals(WalletPlan.Nothing, cards(repo))
    }

    @Test
    fun withNoCardSavedACardCanStillBeSaved() = runTest {
        val repo = FakeAutofillRepository().apply { cardChoices = choices(true, cards = emptyList()) }
        val plan = cards(repo) as WalletPlan.Cards
        assertTrue(plan.rows.isEmpty())
        assertTrue(plan.save)
    }

    private val identityForm = IdentityForm(
        listOf(
            IdentityField(0, IdentityRole.FIRST_NAME),
            IdentityField(1, IdentityRole.CPF),
            IdentityField(2, IdentityRole.CITY),
        ),
        "shop.example.com", "https",
    )
    private val me = IdentityChoice(
        "Samuel Rocha", "user@example.com", listOf(IdentityRole.FIRST_NAME, IdentityRole.CPF),
    )

    @Test
    fun directIdentityValuesNeverIncludeDocuments() = runTest {
        val repo = FakeAutofillRepository().apply {
            identityChoice = Outcome.Ok(me)
            identityValueList = Outcome.Ok(listOf(IdentityValue(IdentityRole.FIRST_NAME, "Samuel")))
        }
        val plan = WalletPlanner.plan(Routed.Identity(identityForm, null), target, true, repo, direct = true)
            as WalletPlan.Identity
        assertEquals(listOf(IdentityRole.FIRST_NAME), plan.roles)
        assertEquals(listOf(IdentityRole.CPF), plan.documents)
        assertEquals(listOf(listOf(IdentityRole.FIRST_NAME) to false), repo.identityAsks)
    }

    @Test
    fun anIdentityWithNothingForThisFormOffersNothing() = runTest {
        val repo = FakeAutofillRepository().apply {
            identityChoice = Outcome.Ok(me.copy(roles = listOf(IdentityRole.PASSPORT)))
        }
        val plan = WalletPlanner.plan(Routed.Identity(identityForm, null), target, true, repo, direct = true)
        assertEquals(WalletPlan.Nothing, plan)
    }

    @Test
    fun aFieldWithAValueIsNeverOverwritten() {
        val fields = listOf(field(hint = "Name on card", isEmpty = false), field(hint = "Card number"))
        val frame = CardFrame(
            null, null,
            listOf(
                CardField(fields[0].index, CardKind.CARDHOLDER_NAME, false),
                CardField(fields[1].index, CardKind.NUMBER, false),
            ),
        )
        val values = listOf(
            listOf(
                CardValue(CardRole.CARDHOLDER_NAME, "Samuel Rocha"),
                CardValue(CardRole.NUMBER, "4111111111111111"),
            ),
        )
        val local = fields.mapIndexed { i, f -> f.copy(index = i) }
        val localFrame = frame.copy(fields = frame.fields.mapIndexed { i, f -> f.copy(index = i) })
        assertEquals(listOf(1), WalletEntries.card(local, listOf(localFrame), values).map { it.first })
        assertEquals(listOf(1), WalletEntries.cardTargets(local, listOf(localFrame)))
    }
}
