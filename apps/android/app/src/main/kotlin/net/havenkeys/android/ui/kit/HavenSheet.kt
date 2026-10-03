package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.AnchoredDraggableDefaults
import androidx.compose.foundation.gestures.AnchoredDraggableState
import androidx.compose.foundation.gestures.DraggableAnchors
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.anchoredDraggable
import androidx.compose.foundation.gestures.animateTo
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.paneTitle
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.ui.window.DialogWindowProvider
import androidx.compose.ui.window.SecureFlagPolicy
import kotlin.math.roundToInt
import kotlinx.coroutines.flow.dropWhile
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.SecureDialogWindow
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

private enum class SheetValue { Hidden, Expanded }

internal const val SHEET_TAG = "haven-sheet"

/** Released past this share of its height, a dragged sheet closes. */
private const val DRAG_DISMISS_FRACTION = 0.4f

/**
 * A bottom sheet (spec §7: springs up, the backdrop dims, drag down or tap
 * outside to close). It lives in a window of its own that sets FLAG_SECURE,
 * drops taps that pass through another app's overlay and is excluded from
 * autofill. [onDismiss] is called once, after the sheet has gone; a close
 * tapped while it is still rising goes straight to closing.
 */
@Composable
fun HavenSheet(
    onDismiss: () -> Unit,
    modifier: Modifier = Modifier,
    title: String? = null,
    content: @Composable ColumnScope.() -> Unit,
) {
    val state = remember { AnchoredDraggableState(SheetValue.Hidden) }
    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    val dismiss by rememberUpdatedState(onDismiss)
    var closing by remember { mutableStateOf(false) }
    val close: () -> Unit = {
        if (!closing) {
            closing = true
            scope.launch {
                state.animateTo(SheetValue.Hidden, motion.springSpec(HavenSprings.sheet))
                dismiss()
            }
        }
    }
    val draggedAway: () -> Unit = {
        if (!closing) {
            closing = true
            dismiss()
        }
    }
    Dialog(
        onDismissRequest = close,
        properties = DialogProperties(
            usePlatformDefaultWidth = false,
            decorFitsSystemWindows = false,
            securePolicy = SecureFlagPolicy.SecureOn,
        ),
    ) {
        SecureDialogWindow(ignoreObscuredTouches = true)
        WindowDim(0f)
        OpenWhenMeasured(state)
        CloseWhenDraggedAway(state, draggedAway)
        SheetFrame(state, close, modifier, title, content)
    }
}

@Composable
private fun SheetFrame(
    state: AnchoredDraggableState<SheetValue>,
    close: () -> Unit,
    modifier: Modifier,
    title: String?,
    content: @Composable ColumnScope.() -> Unit,
) {
    val closeLabel = stringResource(R.string.kit_close)
    val motion = HavenTheme.motion
    Box(Modifier.fillMaxSize()) {
        Box(
            Modifier
                .fillMaxSize()
                .graphicsLayer { alpha = shownFraction(state) }
                .background(HavenTheme.colors.scrim)
                .clickable(
                    interactionSource = null,
                    indication = null,
                    onClickLabel = closeLabel,
                    role = Role.Button,
                    onClick = close,
                )
                .semantics { contentDescription = closeLabel },
        )
        SheetSurface(
            title,
            modifier
                .align(Alignment.BottomCenter)
                .onSizeChanged { size ->
                    state.updateAnchors(
                        DraggableAnchors {
                            SheetValue.Hidden at size.height.toFloat()
                            SheetValue.Expanded at 0f
                        },
                    )
                }
                .offset { IntOffset(0, state.offset.takeUnless { it.isNaN() }?.roundToInt() ?: 0) }
                .graphicsLayer { alpha = if (state.offset.isNaN()) 0f else 1f }
                .anchoredDraggable(
                    state = state,
                    orientation = Orientation.Vertical,
                    flingBehavior = AnchoredDraggableDefaults.flingBehavior(
                        state = state,
                        positionalThreshold = { distance -> distance * DRAG_DISMISS_FRACTION },
                        animationSpec = motion.springSpec(HavenSprings.sheet),
                    ),
                )
                .testTag(SHEET_TAG),
            content,
        )
    }
}

/** The sheet as it draws, without its window: the catalogue shows it inline. */
@Composable
internal fun SheetSurface(title: String?, modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    val colors = HavenTheme.colors
    Column(
        modifier
            .fillMaxWidth()
            .shadow(24.dp, HavenShape.sheet)
            .clip(HavenShape.sheet)
            .background(colors.raised)
            .navigationBarsPadding()
            .padding(start = HavenSpacing.gutter, end = HavenSpacing.gutter, bottom = HavenSpacing.gutter)
            .semantics { if (title != null) paneTitle = title },
    ) {
        Box(
            Modifier
                .align(Alignment.CenterHorizontally)
                .padding(vertical = 10.dp)
                .size(width = 36.dp, height = 5.dp)
                .clip(HavenShape.pill)
                .background(colors.lineStrong),
        )
        if (title != null) {
            HavenText(
                title,
                Modifier.padding(top = 4.dp, bottom = 12.dp).semantics { heading() },
                style = HavenTheme.type.title,
                color = colors.textStrong,
            )
        }
        content()
    }
}

private fun shownFraction(state: AnchoredDraggableState<SheetValue>): Float {
    val hidden = state.anchors.positionOf(SheetValue.Hidden)
    val offset = state.offset
    if (hidden.isNaN() || offset.isNaN() || hidden <= 0f) return 0f
    return (1f - offset / hidden).coerceIn(0f, 1f)
}

/** Rises once its height is known (the anchors need it). */
@Composable
private fun OpenWhenMeasured(state: AnchoredDraggableState<SheetValue>) {
    val motion = HavenTheme.motion
    LaunchedEffect(state) {
        snapshotFlow { state.anchors.size }.first { it > 0 }
        state.animateTo(SheetValue.Expanded, motion.springSpec(HavenSprings.sheet))
    }
}

/** Opened, then settled hidden again without a close: the user dragged it away. */
@Composable
private fun CloseWhenDraggedAway(state: AnchoredDraggableState<SheetValue>, onGone: () -> Unit) {
    val latest by rememberUpdatedState(onGone)
    LaunchedEffect(state) {
        snapshotFlow { state.settledValue }
            .dropWhile { it == SheetValue.Hidden }
            .first { it == SheetValue.Hidden }
        latest()
    }
}

/**
 * Sets how much this dialog window dims the screen behind it. The sheet
 * passes 0 (it draws its own animated scrim; the window's dim would double
 * it); the dialog passes the scrim's alpha, so both darken the screen alike
 * instead of the platform's heavier 60% black.
 */
@Composable
internal fun WindowDim(amount: Float) {
    val view = LocalView.current
    SideEffect { (view.parent as? DialogWindowProvider)?.window?.setDimAmount(amount) }
}

@PreviewLightDark
@Composable
private fun HavenSheetPreview() {
    KitPreview {
        SheetSurface("New item") {
            HavenText("Login")
            HavenText("Secure note")
        }
    }
}
