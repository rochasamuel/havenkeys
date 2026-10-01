package net.havenkeys.android.ui.generator

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Before
import org.junit.Test
import uniffi.havenkeys_mobile.Generated

@OptIn(ExperimentalCoroutinesApi::class)
class GeneratorViewModelTest {
    @Before fun main() = Dispatchers.setMain(UnconfinedTestDispatcher())

    @After fun reset() = Dispatchers.resetMain()

    private val vault = FakeVaultRepository()
    private val settings = FakeSettingsRepository()

    private fun vm() = GeneratorViewModel(vault, settings)

    @Test
    fun startsWithTheDesktopDefaults() {
        val options = vm().state.value.options
        assertEquals(24u, options.length)
        assertEquals(listOf(true, true, true, true, false), options.flags())
    }

    @Test
    fun optionsRoundTripToRust() = runTest {
        val vm = vm()
        val changed = vm.state.value.options.copy(length = 40u, symbols = false, avoidAmbiguous = true)
        vm.setOptions(changed)
        assertEquals(changed, vm.state.value.options)
        vm.generate()
        assertEquals(changed, vault.generatedWith)
    }

    @Test
    fun theLengthIsClampedBeforeRustSeesIt() = runTest {
        val vm = vm()
        vm.setOptions(vm.state.value.options.copy(length = 3u))
        assertEquals(8u, vm.state.value.options.length)
        vm.generate()
        assertEquals(8u, vault.generatedWith?.length)
        vm.setOptions(vm.state.value.options.copy(length = 500u))
        assertEquals(128u, vm.state.value.options.length)
    }

    @Test
    fun generateReturnsThePasswordAndTheStateNeverHoldsIt() = runTest {
        vault.generated = Outcome.Ok(Generated("correct-horse-battery", 97.5))
        val vm = vm()
        val result = vm.generate()
        assertEquals("correct-horse-battery", (result as Outcome.Ok<String>).value)
        assertEquals(97.5, vm.state.value.entropyBits!!, 0.0)
        assertFalse(vm.state.value.toString().contains("correct-horse-battery"))
    }

    @Test
    fun aFailureClearsTheStrength() = runTest {
        val vm = vm()
        vm.generate()
        vault.generated = Outcome.Failed("invalid_input")
        assertEquals(Outcome.Failed("invalid_input"), vm.generate())
        assertNull(vm.state.value.entropyBits)
    }

    @Test
    fun theClipboardDelayComesFromSettings() = runTest {
        settings.current = Outcome.Ok(net.havenkeys.android.fakes.settings().copy(clipboardClearSeconds = 90u))
        assertEquals(90, vm().clipboardClearSeconds())
    }

    private fun uniffi.havenkeys_mobile.GeneratorOptions.flags() =
        listOf(uppercase, lowercase, digits, symbols, avoidAmbiguous)
}
