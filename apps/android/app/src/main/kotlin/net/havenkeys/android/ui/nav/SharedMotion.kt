package net.havenkeys.android.ui.nav

import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.BoundsTransform
import androidx.compose.animation.ExperimentalSharedTransitionApi
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenSprings

/** The search pill in the shell's top bar and the search screen's field are one element. */
internal const val SEARCH_KEY = "search-pill"

/** A row's title and the item screen's title. */
internal fun titleKey(id: String): String = "title-$id"

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

/**
 * Which row's title travels to the item screen: the one tapped last. An
 * item can be in Recently added and Frequently used at once, and two
 * elements with one key would fight. Ids and list names only, in memory.
 */
@Stable
internal class TitleTravel {
    private var tapped: String? by mutableStateOf(null)

    fun tap(id: String, origin: String) {
        tapped = "$origin/$id"
    }

    @OptIn(ExperimentalSharedTransitionApi::class)
    fun from(shared: SharedTransitionScope, visibility: AnimatedVisibilityScope, motion: HavenMotion): SharedTitle =
        { id, origin ->
            if (tapped == "$origin/$id") Modifier.sharedIfMoving(shared, titleKey(id), visibility, motion) else Modifier
        }
}
