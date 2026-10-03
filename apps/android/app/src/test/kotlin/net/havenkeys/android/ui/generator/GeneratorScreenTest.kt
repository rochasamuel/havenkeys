package net.havenkeys.android.ui.generator

import android.content.ClipboardManager
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.StateRestorationTester
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import net.havenkeys.android.R
import net.havenkeys.android.clipboard.SensitiveClipboard
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.fakes.FakeSettingsRepository
import net.havenkeys.android.fakes.FakeVaultRepository
import net.havenkeys.android.ui.kit.setKit
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.havenkeys_mobile.Generated

@RunWith(RobolectricTestRunner::class)
@Config(qualifiers = "w411dp-h2000dp")
class GeneratorScreenTest {
    @get:Rule
    val rule = createComposeRule()

    private val app = RuntimeEnvironment.getApplication()
    private fun text(id: Int, vararg args: Any) = app.getString(id, *args)

    private val vault = FakeVaultRepository().apply { generated = Outcome.Ok(Generated("Pa55-word!", 120.0)) }
    private val clipboard = SensitiveClipboard(app, CoroutineScope(SupervisorJob()))

    private fun vm() = GeneratorViewModel(vault, FakeSettingsRepository())

    private fun show() {
        val vm = vm()
        rule.setKit { GeneratorScreen(vm, clipboard, online = true, onBack = {}, onLock = {}) }
    }

    @Test
    fun thePasswordShowsWithItsStrength() {
        show()
        rule.onNodeWithText("Pa55-word!").assertExists()
        rule.onNodeWithText(text(R.string.generator_strength, text(R.string.generator_excellent), 120)).assertExists()
    }

    @Test
    fun regenerateAsksRustAgain() {
        show()
        vault.generated = Outcome.Ok(Generated("Second-0ne", 120.0))
        rule.onNode(hasText(text(R.string.generator_regenerate)) and hasClickAction()).performClick()
        rule.onNodeWithText("Second-0ne").assertExists()
    }

    @Test
    fun copyPutsItOnTheClipboardAndSaysForHowLong() {
        show()
        rule.onNode(hasText(text(R.string.generator_copy)) and hasClickAction()).performClick()
        rule.waitForIdle()
        val clip = app.getSystemService(ClipboardManager::class.java).primaryClip!!.getItemAt(0).text.toString()
        assertEquals("Pa55-word!", clip)
        rule.onNodeWithText(text(R.string.generator_copied, 30)).assertExists()
        assertEquals(emptyList<String>(), vault.usesRecorded)
    }

    @Test
    fun theLengthReadsAsANumberAndSymbolsCanBeTurnedOff() {
        show()
        val slider = rule.onNodeWithContentDescription(text(R.string.generator_length))
        assertEquals("24", slider.fetchSemanticsNode().config.getOrNull(SemanticsProperties.StateDescription))
        rule.onNode(hasText(text(R.string.generator_symbols)) and hasClickAction()).performClick()
        rule.runOnIdle { assertEquals(false, vault.generatedWith?.symbols) }
    }

    /** Review (stage 4): TalkBack read "Length, 24" from the row and again from the slider; now only the slider. */
    @Test
    fun theLengthIsReadOnceByTheSliderAlone() {
        show()
        rule.onAllNodesWithText(text(R.string.generator_length)).assertCountEquals(0)
        rule.onAllNodesWithText("24").assertCountEquals(0)
        rule.onNodeWithContentDescription(text(R.string.generator_length)).assertExists()
    }

    @Test
    fun aRestoredGeneratorDoesNotBringTheOldPasswordBack() {
        val restoration = StateRestorationTester(rule)
        val vm = vm()
        restoration.setContent { HavenTheme { GeneratorScreen(vm, clipboard, true, {}, {}) } }
        rule.onNodeWithText("Pa55-word!").assertExists()
        vault.generated = Outcome.Ok(Generated("Fresh-0ne", 120.0))
        restoration.emulateSavedInstanceStateRestore()
        rule.onAllNodesWithText("Pa55-word!").assertCountEquals(0)
        rule.onNodeWithText("Fresh-0ne").assertExists()
    }
}
