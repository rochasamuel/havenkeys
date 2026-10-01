package net.havenkeys.android.autofill

import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.TargetFacts
import uniffi.havenkeys_mobile.TargetKind

class FillPlannerTest {
    private val target = TargetFacts("com.android.chrome", listOf(ByteArray(32)), "github.com", "https")
    private val login = LoginForm(listOf(0), listOf(1), emptyList(), "github.com", "https", 0)
    private val otp = LoginForm(emptyList(), emptyList(), listOf(0), "github.com", "https", 0)
    private val match = AutofillMatch("id1", "GitHub", "octo", true)

    @Test
    fun lockedAsksToUnlockFirst() = runTest {
        val repo = FakeAutofillRepository()
        assertEquals(FillPlan.UnlockFirst, FillPlanner.plan(login, target, unlocked = false, repo))
        assertTrue("no vault data is read while locked", repo.calls.isEmpty())
    }

    @Test
    fun directFillCarriesTheValuesOfEachMatch() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            confirm = false
            values = Outcome.Ok(FillValues("octo", "hunter2"))
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals("hunter2", plan.datasets.single().values?.password)
    }

    @Test
    fun confirmBeforeFillingGatesEveryDataset() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            confirm = true
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertNull(plan.datasets.single().values)
        assertNull(plan.datasets.single().totp)
        assertTrue(repo.calls.none { it.startsWith("fill:") || it.startsWith("totp:") })
    }

    @Test
    fun anOtpFieldIsOfferedTheCodeOfMatchesWithTotp() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match, match.copy(id = "id2", hasTotp = false)))
            confirm = false
            code = Outcome.Ok("123456")
        }
        val plan = FillPlanner.plan(otp, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals(listOf("id1"), plan.datasets.map { it.match.id })
        assertEquals("123456", plan.datasets.single().totp)
        assertTrue(repo.calls.none { it.startsWith("fill:") })
    }

    @Test
    fun aRefusedTargetOffersNothing() = runTest {
        val repo = FakeAutofillRepository().apply { kind = Outcome.Failed("denied") }
        assertEquals(FillPlan.Nothing, FillPlanner.plan(login, target, unlocked = true, repo))
        assertEquals(FillPlan.Nothing, FillPlanner.plan(login, target, unlocked = false, repo))
    }

    @Test
    fun aMatchRustRefusesToFillIsNotOffered() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            confirm = false
            values = Outcome.Failed("denied")
        }
        assertEquals(FillPlan.Nothing, FillPlanner.plan(login, target, unlocked = true, repo))
        repo.apply { code = Outcome.Failed("denied") }
        assertEquals(FillPlan.Nothing, FillPlanner.plan(otp, target, unlocked = true, repo))
    }

    @Test
    fun failedMatchesOfferNothing() = runTest {
        val repo = FakeAutofillRepository().apply { matchList = Outcome.Failed("denied") }
        assertEquals(FillPlan.Nothing, FillPlanner.plan(login, target, unlocked = true, repo))
    }

    @Test
    fun aVaultRustLocksOnTheWayAsksToUnlockFirst() = runTest {
        // The auto-lock was overdue: Rust locked the vault on this request.
        val repo = FakeAutofillRepository().apply { matchList = Outcome.Failed("locked") }
        assertEquals(FillPlan.UnlockFirst, FillPlanner.plan(login, target, unlocked = true, repo))
    }

    @Test
    fun offersAtMostFiveMatches() = runTest {
        val many = (1..8).map { match.copy(id = "id$it") }
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(many)
            confirm = true
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals(FillPlanner.MAX_DATASETS, plan.datasets.size)
    }

    @Test
    fun searchIsOfferedToAppsOnly() = runTest {
        val app = target.copy(packageName = "com.github.android", webDomain = null, webScheme = null)
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(emptyList())
            kind = Outcome.Ok(TargetKind.APP)
        }
        assertTrue((FillPlanner.plan(login, app, true, repo) as FillPlan.Offer).search)
        repo.kind = Outcome.Ok(TargetKind.BROWSER)
        assertEquals(FillPlan.Nothing, FillPlanner.plan(login, target, true, repo))
    }

    @Test
    fun searchNeedsALoginField() = runTest {
        val app = target.copy(packageName = "com.github.android", webDomain = null, webScheme = null)
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(emptyList())
            kind = Outcome.Ok(TargetKind.APP)
        }
        assertEquals(FillPlan.Nothing, FillPlanner.plan(otp, app, true, repo))
    }

    @Test
    fun aDatasetPlanNeverPrintsItsValues() {
        val plan = DatasetPlan(match, FillValues("octo", "hunter2"), "123456")
        val text = plan.toString()
        assertFalse(text.contains("hunter2"))
        assertFalse(text.contains("123456"))
        assertFalse(text.contains("octo"))
        assertFalse(FillPlan.Offer(listOf(plan), search = false).toString().contains("hunter2"))
        assertEquals(plan, DatasetPlan(match, FillValues("octo", "hunter2"), "123456"))
    }
}
