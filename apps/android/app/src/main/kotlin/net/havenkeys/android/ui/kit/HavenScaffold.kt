package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.WindowInsetsSides
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.only
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Room under the content for the floating button (56dp + 16dp margin + 16dp air). */
private val FloatingClearance = 88.dp

/**
 * A screen's frame on the window ground: [topBar] under the status bar,
 * the content, [bottomBar] over the navigation bar, the floating button at
 * the bottom end and toasts above the bottom edge. The keyboard pushes the
 * whole frame up. [content] receives the bottom room it must leave for the
 * floating button.
 */
@Composable
fun HavenScaffold(
    modifier: Modifier = Modifier,
    topBar: @Composable () -> Unit = {},
    bottomBar: (@Composable () -> Unit)? = null,
    floatingButton: (@Composable () -> Unit)? = null,
    toastState: ToastState? = null,
    content: @Composable (PaddingValues) -> Unit,
) {
    val clearance = if (floatingButton != null) FloatingClearance else 0.dp
    Column(
        modifier
            .fillMaxSize()
            .background(HavenTheme.colors.pane)
            // Cutouts and a landscape side bar; consumed here, so nothing below pads them twice.
            .windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Horizontal))
            .imePadding(),
    ) {
        Box(Modifier.fillMaxWidth().statusBarsPadding()) { topBar() }
        Box(Modifier.weight(1f).fillMaxWidth()) {
            content(PaddingValues(bottom = clearance))
            if (floatingButton != null) {
                Box(Modifier.align(Alignment.BottomEnd).padding(HavenSpacing.gutter)) { floatingButton() }
            }
            if (toastState != null) {
                ToastHost(toastState, Modifier.align(Alignment.BottomCenter).padding(bottom = clearance + 22.dp))
            }
        }
        if (bottomBar != null) {
            Box(Modifier.fillMaxWidth().navigationBarsPadding()) { bottomBar() }
        } else {
            Spacer(Modifier.navigationBarsPadding())
        }
    }
}

@PreviewLightDark
@Composable
private fun HavenScaffoldPreview() {
    HavenTheme {
        HavenScaffold(
            topBar = { HavenText("Top bar", Modifier.padding(16.dp)) },
            bottomBar = { HavenText("Bottom bar", Modifier.padding(16.dp)) },
            floatingButton = { AddButton(onClick = {}) },
        ) { padding -> HavenText("Content", Modifier.padding(padding).padding(16.dp)) }
    }
}
