@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.nav

import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenSprings

/** How one screen replaces another (spec §7). */
internal enum class Move {
    /** Over the shell, or a category list: in from the right; the old screen shifts left and dims. */
    PUSH,

    /** A tab change: a short crossfade with a few dp of rise; the bars stay. */
    TAB,

    /** Unlock or onboarding to the shell: the slower seal. */
    SEAL,

    /** Search: a plain fade under the pill's shared bounds. */
    FADE,

    /** The lock and the removal wipe: nothing animates (spec §7, Lock). */
    CUT,
}

/** The app's screens that cover the shell, full screen (spec §6.1). */
internal val PUSHED = setOf(
    Routes.ITEM,
    Routes.EDIT,
    Routes.NEW,
    Routes.GENERATOR,
    Routes.DEVICES,
    Routes.PAIRING,
    Routes.AUTOFILL_SETUP,
)

/** The old screen moves this share of its width left under a push (spec §7: about 30%)... */
private const val UNDER_SHIFT = 0.3f

/** ...and fades to this over the window's ground, which reads as dimmed. */
private const val UNDER_ALPHA = 0.6f

/** The move between two of the app's routes; the same pair names a push and its pop. */
internal fun outerMove(from: String?, to: String?): Move = when {
    to == Routes.UNLOCK || to == Routes.ONBOARDING -> Move.CUT
    from == Routes.UNLOCK || from == Routes.ONBOARDING -> Move.SEAL
    to in PUSHED || from in PUSHED -> Move.PUSH
    else -> Move.FADE
}

/** The incoming screen's transition; [pop] when Back reveals it. Every spec comes from [motion]. */
internal fun enterFor(move: Move, motion: HavenMotion, pop: Boolean, tabShiftPx: Int = 0): EnterTransition {
    if (motion.reduced || move == Move.CUT) return EnterTransition.None
    return when (move) {
        Move.PUSH -> if (pop) {
            slideInHorizontally(motion.springSpec(HavenSprings.smooth)) { -(it * UNDER_SHIFT).toInt() } +
                fadeIn(motion.springSpec(HavenSprings.smooth), initialAlpha = UNDER_ALPHA)
        } else {
            slideInHorizontally(motion.springSpec(HavenSprings.smooth)) { it }
        }
        Move.TAB -> fadeIn(motion.fadeSpec()) + slideInVertically(motion.springSpec(HavenSprings.smooth)) { tabShiftPx }
        Move.SEAL -> fadeIn(motion.sealSpec())
        Move.FADE, Move.CUT -> fadeIn(motion.fadeSpec())
    }
}

/** The outgoing screen's transition; [pop] when Back removes it. */
internal fun exitFor(move: Move, motion: HavenMotion, pop: Boolean): ExitTransition {
    if (motion.reduced || move == Move.CUT) return ExitTransition.None
    return when (move) {
        Move.PUSH -> if (pop) {
            slideOutHorizontally(motion.springSpec(HavenSprings.smooth)) { it }
        } else {
            slideOutHorizontally(motion.springSpec(HavenSprings.smooth)) { -(it * UNDER_SHIFT).toInt() } +
                fadeOut(motion.springSpec(HavenSprings.smooth), targetAlpha = UNDER_ALPHA)
        }
        Move.TAB -> fadeOut(motion.fadeSpec())
        Move.SEAL -> fadeOut(motion.sealSpec())
        Move.FADE, Move.CUT -> fadeOut(motion.fadeSpec())
    }
}
