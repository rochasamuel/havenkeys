package net.havenkeys.android.ui.onboarding

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeAccountRepository
import net.havenkeys.android.fakes.status
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.InvitePreview

@OptIn(ExperimentalCoroutinesApi::class)
class OnboardingViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val preview = KitPreview("a@example.com", "https://v.example.com")

    private fun frame() = LumaFrame(1u, 1u, byteArrayOf(7))

    @Test
    fun aScannedKitMovesToThePasswordStepShowingOnlyAddressAndServer() = runTest {
        val accounts = FakeAccountRepository().apply { kit = Outcome.Ok(preview) }
        val vm = OnboardingViewModel(accounts)
        vm.choose(OnboardingUiState.Mode.SCAN)
        vm.onFrame(frame())
        assertEquals(OnboardingUiState.Mode.PASSWORD, vm.state.value.mode)
        assertEquals("a@example.com", vm.state.value.preview?.email)
        assertEquals("https://v.example.com", vm.state.value.preview?.serverUrl)
    }

    @Test
    fun aFrameWithoutAKitKeepsScanning() = runTest {
        val vm = OnboardingViewModel(FakeAccountRepository().apply { kit = Outcome.Ok(null) })
        vm.choose(OnboardingUiState.Mode.SCAN)
        vm.onFrame(frame())
        assertEquals(OnboardingUiState.Mode.SCAN, vm.state.value.mode)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun aForeignKitCodeShowsTheErrorAndKeepsScanning() = runTest {
        val vm = OnboardingViewModel(FakeAccountRepository().apply { kit = Outcome.Failed("invalid_kit") })
        vm.choose(OnboardingUiState.Mode.SCAN)
        vm.onFrame(frame())
        assertEquals(OnboardingUiState.Mode.SCAN, vm.state.value.mode)
        assertEquals("invalid_kit", vm.state.value.errorCode)
    }

    @Test
    fun aFrameIsWipedOnceRustHasIt() = runTest {
        val vm = OnboardingViewModel(FakeAccountRepository())
        vm.choose(OnboardingUiState.Mode.SCAN)
        val scanned = frame()
        vm.onFrame(scanned)
        assertTrue(scanned.bytes.all { it == 0.toByte() })
    }

    @Test
    fun framesOutsideTheScanStepAreNotDecoded() = runTest {
        val accounts = FakeAccountRepository()
        val vm = OnboardingViewModel(accounts)
        val ignored = frame()
        vm.onFrame(ignored)
        assertFalse(accounts.calls.contains("scanKit"))
        assertTrue(ignored.bytes.all { it == 0.toByte() })
    }

    @Test
    fun aWrongPasswordShowsTheCodeAndKeepsTheKit() = runTest {
        val accounts = FakeAccountRepository().apply { nextStatus = Outcome.Failed("sign_in_failed") }
        val vm = OnboardingViewModel(accounts)
        vm.signInWithKit("nope")
        assertEquals("sign_in_failed", vm.state.value.errorCode)
        assertFalse(vm.state.value.busy)
        assertFalse(vm.state.value.done)
        assertFalse(accounts.calls.contains("forgetKit"))
    }

    @Test
    fun leavingThePasswordStepForgetsTheKit() = runTest {
        val accounts = FakeAccountRepository().apply { kit = Outcome.Ok(preview) }
        val vm = OnboardingViewModel(accounts)
        vm.choose(OnboardingUiState.Mode.SCAN)
        vm.onFrame(frame())
        vm.back()
        assertTrue(accounts.calls.contains("forgetKit"))
        assertEquals(OnboardingUiState(), vm.state.value)
    }

    @Test
    fun success_isDone() = runTest {
        val vm = OnboardingViewModel(FakeAccountRepository().apply { nextStatus = Outcome.Ok(status()) })
        vm.signInWithKit("correct horse battery staple")
        assertTrue(vm.state.value.done)
    }

    @Test
    fun typingTheKitSignsIn() = runTest {
        val accounts = FakeAccountRepository()
        val vm = OnboardingViewModel(accounts)
        vm.choose(OnboardingUiState.Mode.TYPE)
        vm.signIn(" https://v.example.com ", "a@example.com", "correct horse battery staple", "H1-AAAA")
        assertEquals(listOf("signIn"), accounts.calls)
        assertTrue(vm.state.value.done)
    }

    @Test
    fun anInviteActivates() = runTest {
        val accounts = FakeAccountRepository().apply { nextStatus = Outcome.Failed("offline") }
        val vm = OnboardingViewModel(accounts)
        vm.choose(OnboardingUiState.Mode.INVITE)
        vm.activate("invite", "correct horse battery staple")
        assertEquals(listOf("activate"), accounts.calls)
        assertEquals("offline", vm.state.value.errorCode)
        assertEquals(OnboardingUiState.Mode.INVITE, vm.state.value.mode)
    }

    @Test
    fun choosingAnotherStepClearsTheError() = runTest {
        val vm = OnboardingViewModel(FakeAccountRepository().apply { nextStatus = Outcome.Failed("offline") })
        vm.activate("invite", "correct horse battery staple")
        vm.choose(OnboardingUiState.Mode.TYPE)
        assertNull(vm.state.value.errorCode)
    }

    @Test
    fun previewShowsTheEmailAndServer() = runTest {
        val accounts = FakeAccountRepository().apply {
            invitePreview = Outcome.Ok(InvitePreview("me@example.com", "https://v.example.com"))
        }
        val vm = OnboardingViewModel(accounts)
        vm.previewInvite("  HKINV1-abc  ")
        assertEquals(listOf("HKINV1-abc"), accounts.previewed)
        assertEquals("me@example.com", vm.state.value.invitePreview?.email)
        assertEquals("https://v.example.com", vm.state.value.invitePreview?.serverUrl)

        vm.previewInvite("something else")
        assertNull(vm.state.value.invitePreview)
        assertEquals("only a setup code reaches Rust", 1, accounts.previewed.size)
    }

    @Test
    fun aRefusedPreviewShowsNothing() = runTest {
        val accounts = FakeAccountRepository().apply { invitePreview = Outcome.Failed("invalid_input") }
        val vm = OnboardingViewModel(accounts)
        vm.previewInvite("HKINV1-nope")
        assertNull(vm.state.value.invitePreview)
    }
}
