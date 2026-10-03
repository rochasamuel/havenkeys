package net.havenkeys.android.ui.kit

import android.view.View
import android.view.WindowManager
import androidx.compose.runtime.Composable
import androidx.compose.ui.test.junit4.createComposeRule
import net.havenkeys.android.ui.theme.DarkHavenColors
import net.havenkeys.android.ui.theme.LightHavenColors
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The kit's own windows are secure: FLAG_SECURE, no taps through another
 * app's overlay, nothing offered to autofill. The windows are read from
 * WindowManagerGlobal: every window a component adds must be secure.
 */
@RunWith(RobolectricTestRunner::class)
class OverlayWindowsTest {
    @get:Rule
    val rule = createComposeRule()

    private class Added(val view: View, val params: WindowManager.LayoutParams)

    private fun windows(): List<Added> {
        val global = Class.forName("android.view.WindowManagerGlobal")
        val instance = global.getMethod("getInstance").invoke(null)
        fun field(name: String): List<*> = global.getDeclaredField(name).run {
            isAccessible = true
            get(instance) as List<*>
        }
        val views = field("mViews")
        val params = field("mParams")
        return views.indices.map { Added(views[it] as View, params[it] as WindowManager.LayoutParams) }
    }

    private fun assertEveryNewWindowIsSecure(show: @Composable () -> Unit) {
        val before = windows().map { it.view }.toSet()
        rule.setKit(content = show)
        rule.waitForIdle()
        val added = windows().filter { it.view !in before }
        assertTrue("the component added a window", added.isNotEmpty())
        added.forEach {
            assertTrue("FLAG_SECURE", it.params.flags and WindowManager.LayoutParams.FLAG_SECURE != 0)
            assertTrue("filterTouchesWhenObscured", it.view.rootView.filterTouchesWhenObscured)
            assertEquals(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS, it.view.rootView.importantForAutofill)
        }
    }

    @Test
    fun theSheetWindowIsSecure() = assertEveryNewWindowIsSecure { HavenSheet(onDismiss = {}) { HavenText("x") } }

    @Test
    fun theDialogWindowIsSecure() = assertEveryNewWindowIsSecure {
        HavenDialog("Title", onDismiss = {}, confirm = DialogAction("Ok", {}))
    }

    @Test
    fun theMenuWindowIsSecure() = assertEveryNewWindowIsSecure {
        HavenMenu(expanded = true, onDismiss = {}, items = listOf(MenuItem("Edit", {})))
    }

    private fun dialogDim(dark: Boolean): Float {
        val before = windows().map { it.view }.toSet()
        rule.setKit(dark = dark) { HavenDialog("Title", onDismiss = {}, confirm = DialogAction("Ok", {})) }
        rule.waitForIdle()
        val params = windows().single { it.view !in before }.view.rootView.layoutParams as WindowManager.LayoutParams
        assertTrue("FLAG_DIM_BEHIND", params.flags and WindowManager.LayoutParams.FLAG_DIM_BEHIND != 0)
        return params.dimAmount
    }

    @Test
    fun aLightDialogDimsTheScreenAsMuchAsTheSheetsScrim() =
        assertEquals(LightHavenColors.scrim.alpha, dialogDim(dark = false), 0.01f)

    @Test
    fun aDarkDialogDimsTheScreenAsMuchAsTheSheetsScrim() =
        assertEquals(DarkHavenColors.scrim.alpha, dialogDim(dark = true), 0.01f)
}
