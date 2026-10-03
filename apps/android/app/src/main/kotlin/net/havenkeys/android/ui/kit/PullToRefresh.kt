package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.AnimationSpec
import androidx.compose.animation.core.animate
import androidx.compose.animation.core.snap
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.HavenTheme

private val PullThreshold = 72.dp
private val PullMax = 120.dp

/** The pull follows the finger at half speed. */
private const val RESISTANCE = 0.5f

/** Where the ring rests while refreshing, as a share of the threshold. */
private const val HOLD_FRACTION = 0.75f

/**
 * Pull down past the top of a scrolling list to refresh (foundation has no
 * pull-to-refresh). The content follows the finger; past 72dp a release
 * calls [onRefresh] once and the ring spins until [refreshing] goes false.
 * TalkBack users get a "Refresh" action on the container instead of a pull.
 */
@Composable
fun PullToRefresh(
    refreshing: Boolean,
    onRefresh: () -> Unit,
    modifier: Modifier = Modifier,
    content: @Composable () -> Unit,
) {
    val density = LocalDensity.current
    val state = remember(density) { with(density) { PullState(PullThreshold.toPx(), PullMax.toPx()) } }
    val settle = HavenTheme.motion.springSpec<Float>(HavenSprings.smooth)
    SideEffect {
        state.onRefresh = onRefresh
        state.settle = settle
        state.refreshing = refreshing
    }
    // The one place the pull animates after a release: it rests at the hold
    // while refreshing, and goes back to 0 as soon as that is not so.
    LaunchedEffect(state.released, refreshing) {
        if (state.released) state.settleTo(if (refreshing) state.hold else 0f)
    }
    val refreshLabel = stringResource(R.string.kit_refresh)
    val refreshingLabel = stringResource(R.string.kit_refreshing)
    Box(
        modifier
            .nestedScroll(state)
            .semantics {
                customActions = listOf(
                    CustomAccessibilityAction(refreshLabel) {
                        if (!refreshing) onRefresh()
                        true
                    },
                )
            },
    ) {
        Box(Modifier.graphicsLayer { translationY = state.pull }) { content() }
        if (refreshing || state.pull > 0f) {
            ProgressRing(
                progress = if (refreshing) null else (state.pull / state.threshold).coerceIn(0f, 1f),
                modifier = Modifier
                    .align(Alignment.TopCenter)
                    .graphicsLayer {
                        translationY = (maxOf(state.pull, if (refreshing) state.hold else 0f) - size.height) / 2
                    },
                contentDescription = if (refreshing) refreshingLabel else null,
            )
        }
    }
}

/** The pull distance, fed by the list's leftover scroll at its top. */
@Stable
private class PullState(val threshold: Float, private val maxPull: Float) : NestedScrollConnection {
    var pull by mutableFloatStateOf(0f)
        private set
    var onRefresh: () -> Unit = {}
    var refreshing = false

    /** The finger has lifted: the pull now settles by itself. */
    var released by mutableStateOf(false)
        private set
    var settle: AnimationSpec<Float> = snap()
    val hold: Float get() = threshold * HOLD_FRACTION

    override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
        // Scrolling back up while pulled: give back the pull first.
        if (source != NestedScrollSource.UserInput || available.y >= 0f || pull <= 0f) return Offset.Zero
        val used = maxOf(available.y, -pull)
        released = false
        pull += used
        return Offset(0f, used)
    }

    override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
        if (source != NestedScrollSource.UserInput || available.y <= 0f) return Offset.Zero
        released = false
        pull = (pull + available.y * RESISTANCE).coerceAtMost(maxPull)
        return Offset(0f, available.y)
    }

    override suspend fun onPreFling(available: Velocity): Velocity {
        if (pull <= 0f) return Velocity.Zero
        if (pull >= threshold && !refreshing) onRefresh()
        released = true
        return available
    }

    suspend fun settleTo(target: Float) {
        animate(pull, target, animationSpec = settle) { value, _ -> pull = value }
    }
}

@PreviewLightDark
@Composable
private fun PullToRefreshPreview() {
    KitPreview {
        PullToRefresh(refreshing = true, onRefresh = {}, modifier = Modifier.height(160.dp)) {
            LazyColumn {
                items(6) { HavenText("Row $it", Modifier.height(48.dp)) }
            }
        }
    }
}
