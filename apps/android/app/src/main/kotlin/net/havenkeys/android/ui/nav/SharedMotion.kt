package net.havenkeys.android.ui.nav

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.BoundsTransform
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenSprings

/** The search pill in the shell's top bar and the search screen's field are one element. */
internal const val SEARCH_KEY = "search-pill"

/** A shared element that moves with its screen; under "Remove animations" nothing is shared and screens cut. */
@OptIn(ExperimentalSharedTransitionApi::class)
@Composable
internal fun Modifier.sharedIfMoving(
    shared: SharedTransitionScope,
    key: String,
    visibility: AnimatedVisibilityScope,
    motion: HavenMotion,
    resize: SharedTransitionScope.ResizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(),
): Modifier = if (motion.reduced) {
    this
} else {
    with(shared) {
        this@sharedIfMoving.sharedBounds(
            rememberSharedContentState(key = key),
            visibility,
            boundsTransform = BoundsTransform { _, _ -> motion.springSpec(HavenSprings.smooth) },
            resizeMode = resize,
        )
    }
}
