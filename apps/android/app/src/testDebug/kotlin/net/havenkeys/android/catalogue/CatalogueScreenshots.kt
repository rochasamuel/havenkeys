package net.havenkeys.android.catalogue

import android.graphics.Bitmap
import android.graphics.Canvas
import androidx.activity.ComponentActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.unit.dp
import java.io.File
import kotlin.math.roundToInt
import net.havenkeys.android.ui.kit.rememberToastState
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

private const val SHOT = "shot"

/**
 * Renders the catalogue to PNGs for design review (apps/android/.impeccable/review).
 * Runs only when the build passes a directory:
 * `./gradlew testGithubDebugUnitTest --tests '*CatalogueScreenshots*' -PscreensDir=.impeccable/review`.
 * Native graphics, so text is measured and drawn by the real engine. The sections
 * are drawn on a tall canvas so each fits one image; the frame uses a phone's height.
 */
@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
class CatalogueScreenshots {
    @get:Rule
    val rule = createAndroidComposeRule<ComponentActivity>()

    private val dir: File? = System.getProperty("havenkeys.screens.dir")?.takeIf { it.isNotBlank() }?.let(::File)

    /**
     * Draws the window into a bitmap (captureToImage waits for a frame commit
     * Robolectric never sends) and keeps [region], or the whole window.
     */
    private fun save(name: String, region: Rect? = null) {
        rule.waitForIdle()
        val view = rule.activity.window.decorView
        val whole = Bitmap.createBitmap(view.width, view.height, Bitmap.Config.ARGB_8888)
        view.draw(Canvas(whole))
        val shot = region?.let {
            val top = it.top.roundToInt().coerceIn(0, view.height - 1)
            val bottom = it.bottom.roundToInt().coerceIn(top + 1, view.height)
            Bitmap.createBitmap(whole, 0, top, view.width, bottom - top)
        } ?: whole
        val out = File(dir, "$name.png")
        out.parentFile?.mkdirs()
        out.outputStream().use { shot.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    @Composable
    private fun Page(dark: Boolean, content: @Composable () -> Unit) {
        HavenTheme(darkTheme = dark) {
            CompositionLocalProvider(LocalCatalogueToasts provides rememberToastState()) {
                Box(
                    Modifier.fillMaxWidth().wrapContentHeight(Alignment.Top, unbounded = true).testTag(SHOT)
                        .background(HavenTheme.colors.pane)
                        .padding(horizontal = HavenSpacing.gutter, vertical = 20.dp),
                ) { content() }
            }
        }
    }

    @Test
    @Config(qualifiers = "w411dp-h2400dp-xxhdpi")
    fun everySectionInBothThemes() {
        assumeTrue(dir != null)
        var dark by mutableStateOf(false)
        var index by mutableIntStateOf(0)
        val sections = catalogueSections()
        rule.setContent { Page(dark) { sections[index].content() } }
        for (theme in listOf(false, true)) {
            sections.forEachIndexed { i, section ->
                dark = theme
                index = i
                rule.waitForIdle()
                val name = "kit-${if (theme) "dark" else "light"}-${section.title.lowercase()}"
                save(name, rule.onNodeWithTag(SHOT).fetchSemanticsNode().boundsInWindow)
            }
        }
    }

    @Test
    @Config(qualifiers = "w411dp-h891dp-xxhdpi")
    fun theFrameWithAToastInBothThemes() {
        assumeTrue(dir != null)
        rule.setContent { KitCatalogue() }
        for (theme in listOf("Light", "Dark")) {
            rule.onNodeWithText(theme).performClick()
            rule.waitForIdle()
            rule.mainClock.autoAdvance = false
            // The add button shows a toast in the catalogue.
            rule.onNodeWithContentDescription("New item").performClick()
            rule.mainClock.advanceTimeBy(800)
            save("kit-${theme.lowercase()}-frame")
            rule.mainClock.autoAdvance = true
            rule.mainClock.advanceTimeBy(4_000)
        }
    }
}
