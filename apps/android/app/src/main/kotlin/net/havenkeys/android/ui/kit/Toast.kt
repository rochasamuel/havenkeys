package net.havenkeys.android.ui.kit

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.border
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalAccessibilityManager
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

/** The glass's 1dp rim: the desktop's rgba(226, 234, 230, 0.1), in both themes. */
private val GlassRim = Color(0x1AE2EAE6)

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
    // The system's "Time to take action" lengthens it for TalkBack and switch users.
    val accessibility = LocalAccessibilityManager.current
    LaunchedEffect(message) {
        if (message != null) {
            delay(
                accessibility?.calculateRecommendedTimeoutMillis(
                    TOAST_MILLIS,
                    containsIcons = true,
                    containsText = true,
                ) ?: TOAST_MILLIS,
            )
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
            // The desktop's rim (styles.css .toast): without it the dark glass
            // vanishes into the dark window ground.
            .border(1.dp, GlassRim, HavenShape.pill)
            .padding(start = 13.dp, end = 16.dp, top = 10.dp, bottom = 10.dp)
            .semantics(mergeDescendants = true) { liveRegion = when (message.tone) {
                    ToastTone.Done -> LiveRegionMode.Polite
                    ToastTone.Alert -> LiveRegionMode.Assertive
                }
            },
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
