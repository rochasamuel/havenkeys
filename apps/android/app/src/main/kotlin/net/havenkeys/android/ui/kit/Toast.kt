package net.havenkeys.android.ui.kit

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.ui.theme.DarkHavenColors
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

/** How long a toast stays. */
internal const val TOAST_MILLIS = 2_500L

enum class ToastTone { Done, Alert }

/** One toast; [id] tells two identical messages apart, so the second one shows too. */
@Immutable
data class ToastMessage(val text: String, val tone: ToastTone, val id: Long)

/**
 * The glass pill's queue of one: a new toast replaces the current one. A
 * toast never carries a secret: callers say what happened ("Password
 * copied · clears in 30 s"), never the value.
 */
@Stable
class ToastState {
    var current: ToastMessage? by mutableStateOf(null)
        private set
    private var next = 0L

    fun show(text: String, tone: ToastTone = ToastTone.Done) {
        current = ToastMessage(text, tone, next++)
    }

    internal fun expire(message: ToastMessage) {
        if (current == message) current = null
    }
}

@Composable
fun rememberToastState(): ToastState = remember { ToastState() }

/** Where toasts appear; HavenScaffold places one above its bottom edge. */
@Composable
fun ToastHost(state: ToastState, modifier: Modifier = Modifier) {
    val motion = HavenTheme.motion
    val message = state.current
    LaunchedEffect(message) {
        if (message != null) {
            delay(TOAST_MILLIS)
            state.expire(message)
        }
    }
    AnimatedContent(
        targetState = message,
        modifier = modifier,
        transitionSpec = {
            val enter = fadeIn(motion.fadeSpec()) +
                slideInVertically(motion.springSpec(HavenSprings.toast)) { it / 2 }
            enter togetherWith fadeOut(motion.fadeSpec())
        },
        label = "toast",
    ) { shown ->
        if (shown != null) ToastPill(shown)
    }
}

@Composable
private fun ToastPill(message: ToastMessage) {
    val colors = HavenTheme.colors
    // The glass is dark in both themes, so its glyphs take the dark theme's inks.
    val (icon, tint) = when (message.tone) {
        ToastTone.Done -> HavenIcon.Check to DarkHavenColors.ok
        ToastTone.Alert -> HavenIcon.Alert to DarkHavenColors.danger
    }
    Row(
        Modifier
            .padding(horizontal = HavenSpacing.gutter)
            .shadow(16.dp, HavenShape.pill)
            .clip(HavenShape.pill)
            .background(colors.glass)
            .padding(start = 13.dp, end = 16.dp, top = 10.dp, bottom = 10.dp)
            .semantics(mergeDescendants = true) { liveRegion = LiveRegionMode.Polite },
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconGlyph(icon, contentDescription = null, tint = tint, size = 18.dp)
        Spacer(Modifier.width(8.dp))
        HavenText(message.text, style = HavenTheme.type.body, color = colors.onGlass)
    }
}

@PreviewLightDark
@Composable
private fun ToastPreview() {
    KitPreview {
        ToastPill(ToastMessage("Password copied · clears in 30 s", ToastTone.Done, 0))
        ToastPill(ToastMessage("HavenKeys is offline", ToastTone.Alert, 1))
    }
}
