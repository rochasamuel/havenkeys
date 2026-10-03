package net.havenkeys.android.ui.shell

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.text.TextLayoutResult
import net.havenkeys.android.ui.kit.setKit
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

/** The top bar on the narrowest common phone; real text measurement, so native graphics. */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(qualifiers = "w360dp-h780dp")
class ShellTopBarNarrowTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun offlineOnA360dpPhoneThePillStillReadsOnOneLine() {
        rule.setKit { ShellTopBar(online = false, syncing = false, actions = TopBarActions({}, {}, {})) }
        val results = mutableListOf<TextLayoutResult>()
        rule.onNodeWithText("Search HavenKeys", useUnmergedTree = true).fetchSemanticsNode()
            .config.getOrNull(SemanticsActions.GetTextLayoutResult)?.action?.invoke(results)
        assertEquals(1, results.first().lineCount)
    }
}
