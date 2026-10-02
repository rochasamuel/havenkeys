package net.havenkeys.android.credentials

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeCredentialRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.CredentialCaller
import uniffi.havenkeys_mobile.PasskeyCreatePlan

@OptIn(ExperimentalCoroutinesApi::class)
class PasskeyCreateViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val caller = CredentialCaller("com.android.chrome", listOf(ByteArray(32)), "https://github.com")
    private val home = AutofillMatch("id1", "GitHub", "octo", false)

    private fun vm(repo: FakeCredentialRepository, conditional: Boolean = false) =
        PasskeyCreateViewModel(repo, caller, "{}", conditional).also { it.load() }

    @Test
    fun thePlanPreselectsTheFirstHome() = runTest {
        val repo = FakeCredentialRepository().apply {
            plan = Outcome.Ok(PasskeyCreatePlan("github.com", "octo", false, listOf(home)))
        }
        val s = vm(repo).state.value
        assertFalse(s.loading)
        assertEquals("github.com", s.rpId)
        assertEquals("id1", s.selected)
    }

    @Test
    fun withoutHomesANewLoginIsChosen() = runTest {
        val repo = FakeCredentialRepository().apply {
            plan = Outcome.Ok(PasskeyCreatePlan("github.com", "octo", false, emptyList()))
        }
        assertNull(vm(repo).state.value.selected)
    }

    @Test
    fun savingSendsTheChosenLoginAndHandsBackTheResponse() = runTest {
        val repo = FakeCredentialRepository().apply {
            plan = Outcome.Ok(PasskeyCreatePlan("github.com", "octo", false, listOf(home)))
            created = Outcome.Ok("""{"type":"public-key"}""")
        }
        val vm = vm(repo)
        vm.select(null)
        vm.create()
        assertEquals("passkeyCreate:new", repo.calls.last())
        assertEquals("""{"type":"public-key"}""", vm.state.value.response)
    }

    @Test
    fun anExcludedAccountIsShownAndNotSaved() = runTest {
        val repo = FakeCredentialRepository().apply {
            plan = Outcome.Ok(PasskeyCreatePlan("github.com", "octo", true, emptyList()))
        }
        val vm = vm(repo)
        assertTrue(vm.state.value.excluded)
        vm.create()
        assertTrue(repo.calls.none { it.startsWith("passkeyCreate:") })
    }

    @Test
    fun aRefusalKeepsTheScreenWithItsCode() = runTest {
        val repo = FakeCredentialRepository().apply {
            plan = Outcome.Ok(PasskeyCreatePlan("github.com", "octo", false, listOf(home)))
            created = Outcome.Failed("offline")
        }
        val vm = vm(repo)
        vm.create()
        assertEquals("offline", vm.state.value.error)
        assertFalse(vm.state.value.busy)
        assertNull(vm.state.value.response)
    }

    @Test
    fun aConditionalCreateIsRefusedWithoutAskingRust() = runTest {
        val repo = FakeCredentialRepository()
        val s = vm(repo, conditional = true).state.value
        assertEquals("denied", s.error)
        assertTrue(repo.calls.isEmpty())
    }
}
