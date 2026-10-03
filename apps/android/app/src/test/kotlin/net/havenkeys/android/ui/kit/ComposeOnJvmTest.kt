package net.havenkeys.android.ui.kit

import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.unit.dp
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/** The harness the kit's tests stand on: Compose, semantics and the theme on the JVM. */
@RunWith(RobolectricTestRunner::class)
class ComposeOnJvmTest {
    @get:Rule
    val rule = createComposeRule()

    @Test
    fun aThemedComposableRendersWithItsSemantics() {
        rule.setKit {
            BasicText("HavenKeys", Modifier.size(48.dp).semantics { role = Role.Button })
        }
        rule.onNodeWithText("HavenKeys").assertIsDisplayed().assert(hasRole(Role.Button)).assertTouchTarget()
    }
}
