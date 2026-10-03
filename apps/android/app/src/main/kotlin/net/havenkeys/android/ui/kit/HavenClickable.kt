package net.havenkeys.android.ui.kit

import androidx.compose.foundation.clickable
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role

/**
 * The kit's clickable: [HavenPress] feedback, no ripple, no interaction
 * source of our own. Put it after the clip and before the background.
 */
internal fun Modifier.havenClickable(
    enabled: Boolean = true,
    role: Role = Role.Button,
    onClickLabel: String? = null,
    onClick: () -> Unit,
): Modifier = clickable(
    interactionSource = null,
    indication = HavenPress,
    enabled = enabled,
    onClickLabel = onClickLabel,
    role = role,
    onClick = onClick,
)
