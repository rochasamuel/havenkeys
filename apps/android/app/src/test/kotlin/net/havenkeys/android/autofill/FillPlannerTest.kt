package net.havenkeys.android.autofill

import kotlinx.coroutines.test.runTest
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAutofillRepository
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.havenkeys_mobile.AutofillKind
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.BankFillRecord
import uniffi.havenkeys_mobile.BankSummaryRecord
import uniffi.havenkeys_mobile.FillValues
import uniffi.havenkeys_mobile.TargetFacts
import uniffi.havenkeys_mobile.TargetKind

class FillPlannerTest {
    private val target = TargetFacts("com.android.chrome", listOf(ByteArray(32)), "github.com", "https")
    private val login = LoginForm(listOf(0), listOf(1), emptyList(), "github.com", "https", 0)
    private val otp = LoginForm(emptyList(), emptyList(), listOf(0), "github.com", "https", 0)
    private val match = AutofillMatch("id1", "GitHub", "octo", true, AutofillKind.LOGIN)

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
    fun eachOfferedCodeCanAlsoBeCopied() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match, match.copy(id = "id2"), match.copy(id = "id3", hasTotp = false)))
            confirm = true
        }
        val plan = FillPlanner.plan(otp, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals(listOf("id1", "id2"), plan.copies.map { it.id })
        assertTrue("no code is read until the copy is tapped", repo.calls.none { it.startsWith("totp:") })
    }

    @Test
    fun aLoginStepOffersNothingToCopy() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            confirm = true
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertTrue(plan.copies.isEmpty())
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
        assertOnlyThePinnedIcon(FillPlanner.plan(login, target, unlocked = true, repo))
        repo.apply { code = Outcome.Failed("denied") }
        assertOnlyThePinnedIcon(FillPlanner.plan(otp, target, unlocked = true, repo))
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
        assertOnlyThePinnedIcon(FillPlanner.plan(login, target, true, repo))
    }

    @Test
    fun searchNeedsALoginField() = runTest {
        val app = target.copy(packageName = "com.github.android", webDomain = null, webScheme = null)
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(emptyList())
            kind = Outcome.Ok(TargetKind.APP)
        }
        assertOnlyThePinnedIcon(FillPlanner.plan(otp, app, true, repo))
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

    @Test
    fun aLoginWithNoMatchStillOffersToSave() = runTest {
        val repo = FakeAutofillRepository()
        val plan = FillPlanner.plan(login, target, unlocked = true, repo, saveable = true) as FillPlan.Offer
        assertTrue(plan.save)
        assertTrue(plan.datasets.isEmpty())
    }

    @Test
    fun aSignUpFormIsOfferedOnlyTheSaveAndReadsNoLogin() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            values = Outcome.Ok(FillValues("octo", "hunter2"))
        }
        val plan = FillPlanner.plan(null, target, unlocked = true, repo, saveable = true) as FillPlan.Offer
        assertTrue(plan.save)
        assertTrue(plan.datasets.isEmpty())
        assertTrue(
            "no login is read for a form with nothing to fill",
            repo.calls.none { it.startsWith("fill:") || it == "matches" },
        )
    }

    @Test
    fun lockedOffersNoSave() = runTest {
        val repo = FakeAutofillRepository()
        assertEquals(FillPlan.UnlockFirst, FillPlanner.plan(login, target, unlocked = false, repo, saveable = true))
        assertEquals(FillPlan.Nothing, FillPlanner.plan(null, target, unlocked = false, repo, saveable = true))
    }

    @Test
    fun withoutAnythingToSaveNothingChanges() = runTest {
        val repo = FakeAutofillRepository()
        assertOnlyThePinnedIcon(FillPlanner.plan(login, target, unlocked = true, repo))
        assertEquals(FillPlan.Nothing, FillPlanner.plan(null, target, unlocked = true, repo))
    }

    @Test
    fun aFrozenAccountOffersNothingAndSavesNothing() = runTest {
        val form = Routed.Login(null, null)
        val repo = FakeAutofillRepository()
        assertEquals(form, offered(form, "com.android.chrome", repo))
        repo.frozen = true
        assertNull(offered(form, "com.android.chrome", repo))
        repo.frozen = false
        assertNull(offered(null, "com.android.chrome", repo))
        assertNull(offered(form, "", repo))
    }

    @Test
    fun aBrowserLookAlikeWithNoMatchesIsOfferedTheWarning() = runTest {
        val repo = FakeAutofillRepository().apply { lookalikeSite = Outcome.Ok("github.com") }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals("github.com", plan.lookalikeOf)
        assertTrue(plan.datasets.isEmpty())
        assertTrue(repo.calls.none { it.startsWith("fill:") || it.startsWith("totp:") })
    }

    @Test
    fun matchesMeanNoLookAlikeQuestion() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match))
            confirm = true
            lookalikeSite = Outcome.Ok("github.com")
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, repo) as FillPlan.Offer
        assertNull(plan.lookalikeOf)
        assertFalse("lookalike" in repo.calls)
    }

    @Test
    fun appsAreNeverAskedAndAFailedAnswerIsNoWarning() = runTest {
        val app = FakeAutofillRepository().apply {
            kind = Outcome.Ok(TargetKind.APP)
            lookalikeSite = Outcome.Ok("github.com")
        }
        val plan = FillPlanner.plan(login, target, unlocked = true, app) as FillPlan.Offer
        assertNull(plan.lookalikeOf)
        assertFalse("lookalike" in app.calls)
        val failed = FakeAutofillRepository().apply { lookalikeSite = Outcome.Failed("locked") }
        assertOnlyThePinnedIcon(FillPlanner.plan(login, target, unlocked = true, failed))
    }

    @Test
    fun aSaveOnlyScreenOnALookAlikeCarriesTheWarningForTheSaveSheet() = runTest {
        val repo = FakeAutofillRepository().apply { lookalikeSite = Outcome.Ok("github.com") }
        val plan = FillPlanner.plan(null, target, unlocked = true, repo, saveable = true) as FillPlan.Offer
        assertEquals("github.com", plan.lookalikeOf)
        assertTrue(plan.save)
    }

    private val bankMatch = AutofillMatch(
        "b1", "Itaú", null, false, AutofillKind.BANK_ACCOUNT, BankSummaryRecord("Itaú", "1234", "45-6"),
    )
    private val bankValues = FillValues(null, "s3cret", BankFillRecord("1234", null, "12345", "6"))

    /** Agência, conta and dígito (Bradesco), then the password; [focus] is focused. */
    private fun bankScreen(focus: Int): List<FieldFacts> = listOf(
        field(html = mapOf("type" to "number", "maxlength" to "4", "label" to "Agência")),
        field(html = mapOf("type" to "number", "maxlength" to "7", "label" to "Conta")),
        field(html = mapOf("type" to "number", "maxlength" to "1", "label" to "Dígito")),
        field(html = mapOf("type" to "password", "label" to "Senha")),
    ).mapIndexed { i, f -> f.copy(focused = i == focus) }

    @Test
    fun bankDatasetCarriesEveryBankBoxAndPassword() = runTest {
        val fields = bankScreen(focus = 0)
        val form = requireNotNull(LoginFormFinder.find(fields))
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match, bankMatch))
            confirm = false
            values = Outcome.Ok(bankValues)
        }
        val plan = FillPlanner.plan(form, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals("a bank box lists bank accounts first", listOf("b1", "id1"), plan.datasets.map { it.match.id })
        val ids = fields.map { it.index }
        assertEquals(ids, FillPlanner.fieldsOf(form, bankMatch))
        val entries = FillPlanner.entries(form, fields, bankValues)
        assertEquals(ids.zip(listOf("1234", "12345", "6", "s3cret")), entries)
    }

    @Test
    fun loginOnBankBoxPutsUsernameInFocusedBox() = runTest {
        val fields = bankScreen(focus = 1)
        val form = requireNotNull(LoginFormFinder.find(fields))
        assertEquals(listOf(fields[1].index, fields[3].index), FillPlanner.fieldsOf(form, match))
        val entries = FillPlanner.entries(form, fields, FillValues("12345", "s3cret"))
        assertEquals(listOf(fields[1].index to "12345", fields[3].index to "s3cret"), entries)
    }

    @Test
    fun aLoginsUsernameNeverGoesIntoADigitBox() = runTest {
        val fields = bankScreen(focus = 2)
        val form = requireNotNull(LoginFormFinder.find(fields))
        assertEquals(listOf(fields[3].index), FillPlanner.fieldsOf(form, match))
        val entries = FillPlanner.entries(form, fields, FillValues("12345", "s3cret"))
        assertEquals(listOf(fields[3].index to "s3cret"), entries)
    }

    @Test
    fun aUsernameLongerThanTheFocusedBankBoxIsNotSet() = runTest {
        // The agência box takes 4 characters: an old login's conta does not fit.
        val fields = bankScreen(focus = 0)
        val form = requireNotNull(LoginFormFinder.find(fields))
        val entries = FillPlanner.entries(form, fields, FillValues("12345", "s3cret"))
        assertEquals(listOf(fields[3].index to "s3cret"), entries)
        assertEquals(fields[0].index to "1234", FillPlanner.entries(form, fields, FillValues("1234", null)).single())
    }

    @Test
    fun aPasswordBoxListsLoginsThenBankAccounts() = runTest {
        val form = requireNotNull(LoginFormFinder.find(bankScreen(focus = 3)))
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(bankMatch, match))
            confirm = true
        }
        val plan = FillPlanner.plan(form, target, unlocked = true, repo) as FillPlan.Offer
        assertEquals(listOf("id1", "b1"), plan.datasets.map { it.match.id })
        val alone = FillPlanner.plan(login.copy(usernames = emptyList()), target, true, repo) as FillPlan.Offer
        assertEquals(listOf("id1", "b1"), alone.datasets.map { it.match.id })
    }

    @Test
    fun aUsernameOrCodeStepIsOfferedNoBankAccount() = runTest {
        val repo = FakeAutofillRepository().apply {
            matchList = Outcome.Ok(listOf(match, bankMatch))
            confirm = true
        }
        val usernameOnly = login.copy(passwords = emptyList())
        for (form in listOf(usernameOnly, otp)) {
            val plan = FillPlanner.plan(form, target, true, repo) as FillPlan.Offer
            assertEquals(listOf("id1"), plan.datasets.map { it.match.id })
        }
    }

    /** A form with nothing to offer still gets an answer: HavenKeys' pinned icon, which reads nothing. */
    private fun assertOnlyThePinnedIcon(plan: FillPlan) {
        val offer = plan as FillPlan.Offer
        assertTrue(offer.datasets.isEmpty() && offer.copies.isEmpty())
        assertTrue(!offer.search && !offer.save && offer.lookalikeOf == null)
    }
}
