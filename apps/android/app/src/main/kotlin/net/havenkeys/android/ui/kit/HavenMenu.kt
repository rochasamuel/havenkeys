package net.havenkeys.android.ui.kit

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.core.MutableTransitionState
import androidx.compose.animation.fadeIn
import androidx.compose.animation.scaleIn
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupPositionProvider
import androidx.compose.ui.window.PopupProperties
import androidx.compose.ui.window.SecureFlagPolicy
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

private val MenuGap = 4.dp

/**
 * A small menu under its trigger (put both in one Box). Its window sets
 * FLAG_SECURE, drops taps through another app's overlay and takes focus so
 * Back and a tap outside close it. Picking an item closes the menu, then acts.
 */
@Composable
fun HavenMenu(expanded: Boolean, onDismiss: () -> Unit, items: List<MenuItem>, modifier: Modifier = Modifier) {
    if (!expanded) return
    val gap = with(LocalDensity.current) { MenuGap.roundToPx() }
    Popup(
        popupPositionProvider = remember(gap) { BelowAnchor(gap) },
        onDismissRequest = onDismiss,
        properties = PopupProperties(focusable = true, securePolicy = SecureFlagPolicy.SecureOn),
    ) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        MenuSurface(items, onDismiss, modifier)
    }
}

/** The menu as it draws, without its window: the catalogue shows it inline. */
@Composable
internal fun MenuSurface(items: List<MenuItem>, onDismiss: () -> Unit, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    val motion = HavenTheme.motion
    val shown = remember { MutableTransitionState(false).apply { targetState = true } }
    AnimatedVisibility(
        visibleState = shown,
        enter = fadeIn(motion.fadeSpec()) +
            scaleIn(
                motion.springSpec(HavenSprings.sheet),
                initialScale = 0.96f,
                transformOrigin = TransformOrigin(1f, 0f),
            ),
    ) {
        Column(
            modifier
                .widthIn(min = 200.dp, max = 280.dp)
                .width(IntrinsicSize.Max)
                .shadow(16.dp, HavenShape.output)
                .clip(HavenShape.output)
                .background(colors.raised)
                .border(1.dp, colors.lineStrong, HavenShape.output)
                .padding(vertical = 6.dp),
        ) {
            items.forEach { item -> MenuRow(item, onDismiss) }
        }
    }
}

/** One action in a [HavenMenu]; [danger] draws it in ember. */
class MenuItem(val label: String, val onClick: () -> Unit, val icon: HavenIcon? = null, val danger: Boolean = false)

@Composable
private fun MenuRow(item: MenuItem, onDismiss: () -> Unit) {
    val colors = HavenTheme.colors
    Row(
        Modifier
            .fillMaxWidth()
            .heightIn(min = HavenSpacing.touch)
            .havenClickable {
                onDismiss()
                item.onClick()
            }
            .padding(horizontal = 14.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (item.icon != null) {
            IconGlyph(
                item.icon,
                contentDescription = null,
                tint = if (item.danger) colors.danger else colors.muted,
                size = 20.dp,
            )
            Spacer(Modifier.width(12.dp))
        }
        HavenText(
            item.label,
            style = HavenTheme.type.body,
            color = if (item.danger) colors.danger else colors.textStrong,
        )
    }
}

/** Below the anchor and end-aligned with it; above it when the window has no room below. */
private class BelowAnchor(private val gap: Int) : PopupPositionProvider {
    override fun calculatePosition(
        anchorBounds: IntRect,
        windowSize: IntSize,
        layoutDirection: LayoutDirection,
        popupContentSize: IntSize,
    ): IntOffset {
        val start = if (layoutDirection == LayoutDirection.Ltr) {
            anchorBounds.right - popupContentSize.width
        } else {
            anchorBounds.left
        }
        val x = start.coerceIn(0, (windowSize.width - popupContentSize.width).coerceAtLeast(0))
        val below = anchorBounds.bottom + gap
        val y = if (below + popupContentSize.height <= windowSize.height) {
            below
        } else {
            (anchorBounds.top - gap - popupContentSize.height).coerceAtLeast(0)
        }
        return IntOffset(x, y)
    }
}

@PreviewLightDark
@Composable
private fun HavenMenuPreview() {
    KitPreview {
        MenuSurface(
            listOf(
                MenuItem("Edit", {}, HavenIcon.Edit),
                MenuItem("Copy username", {}, HavenIcon.Copy),
                MenuItem("Delete", {}, HavenIcon.Trash, danger = true),
            ),
            onDismiss = {},
        )
    }
}
