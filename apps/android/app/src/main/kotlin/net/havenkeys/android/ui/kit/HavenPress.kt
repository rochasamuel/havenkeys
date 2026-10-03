package net.havenkeys.android.ui.kit

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.node.CompositionLocalConsumerModifierNode
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.currentValueOf
import kotlinx.coroutines.launch
import net.havenkeys.android.ui.theme.HavenSprings
import net.havenkeys.android.ui.theme.LocalHavenColors
import net.havenkeys.android.ui.theme.LocalHavenMotion
import net.havenkeys.android.ui.theme.PRESS_SCALE

/**
 * The kit's only press feedback (spec §5.2: no ripple anywhere). While
 * pressed, the element scales to 0.97 and takes a brass-soft wash, on the
 * press spring; under "Remove animations" both cut. Put it after the clip
 * and before the background, so the background scales with the content:
 * `Modifier.clip(shape).clickable(interactionSource = null, indication = HavenPress) { … }.background(…)`.
 */
object HavenPress : IndicationNodeFactory {
    override fun create(interactionSource: InteractionSource): DelegatableNode = PressNode(interactionSource)

    override fun equals(other: Any?): Boolean = other === this

    override fun hashCode(): Int = javaClass.hashCode()
}

private class PressNode(private val source: InteractionSource) :
    Modifier.Node(),
    DrawModifierNode,
    CompositionLocalConsumerModifierNode {
    private val scale = Animatable(1f)
    private val wash = Animatable(0f)

    override fun onAttach() {
        coroutineScope.launch {
            var held = 0
            source.interactions.collect { interaction ->
                when (interaction) {
                    is PressInteraction.Press -> held++
                    is PressInteraction.Release, is PressInteraction.Cancel -> held = (held - 1).coerceAtLeast(0)
                    else -> return@collect
                }
                settle(pressed = held > 0)
            }
        }
    }

    private fun settle(pressed: Boolean) {
        val spec = currentValueOf(LocalHavenMotion).springSpec<Float>(HavenSprings.press)
        coroutineScope.launch { scale.animateTo(if (pressed) PRESS_SCALE else 1f, spec) }
        coroutineScope.launch { wash.animateTo(if (pressed) 1f else 0f, spec) }
    }

    override fun ContentDrawScope.draw() {
        val tint = currentValueOf(LocalHavenColors).brassSoft
        scale(scale.value) {
            this@draw.drawContent()
            if (wash.value > 0f) drawRect(tint, alpha = wash.value)
        }
    }
}
