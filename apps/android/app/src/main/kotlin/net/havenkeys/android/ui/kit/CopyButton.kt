package net.havenkeys.android.ui.kit

import androidx.compose.animation.Crossfade
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** How long the check stays after a copy. */
internal const val COPIED_MILLIS = 1_500L

/**
 * Copies one field: the glyph turns into a green check for a moment, with a
 * light haptic (spec §7). The caller does the copy, its toast and clearing
 * the clipboard. The label names the field ("Copy Password"), never its value.
 */
@Composable
fun CopyButton(fieldLabel: String, onCopy: () -> Unit, modifier: Modifier = Modifier) {
    var copied by remember { mutableStateOf(false) }
    val haptics = LocalHapticFeedback.current
    val colors = HavenTheme.colors
    val label = stringResource(R.string.copy, fieldLabel)
    val done = stringResource(R.string.kit_copied)
    LaunchedEffect(copied) {
        if (copied) {
            delay(COPIED_MILLIS)
            copied = false
        }
    }
    Box(
        modifier = modifier
            .size(HavenSpacing.touch)
            .clip(CircleShape)
            .havenClickable {
                onCopy()
                haptics.performHapticFeedback(HapticFeedbackType.Confirm)
                copied = true
            }
            .semantics {
                contentDescription = label
                if (copied) stateDescription = done
            },
        contentAlignment = Alignment.Center,
    ) {
        Crossfade(targetState = copied, animationSpec = HavenTheme.motion.fadeSpec(), label = "copy") { isDone ->
            IconGlyph(
                if (isDone) HavenIcon.Check else HavenIcon.Copy,
                contentDescription = null,
                tint = if (isDone) colors.ok else colors.muted,
                size = 20.dp,
            )
        }
    }
}

@PreviewLightDark
@Composable
private fun CopyButtonPreview() {
    KitPreview { CopyButton("Password", onCopy = {}) }
}
