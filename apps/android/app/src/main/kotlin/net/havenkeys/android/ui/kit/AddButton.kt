package net.havenkeys.android.ui.kit

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.tooling.preview.PreviewLightDark
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.theme.HavenTheme

private val AddSize = 56.dp

/**
 * The floating add button (spec §6.6): the primary colour, a plus, a soft
 * shadow because it floats. The screen places it; HavenScaffold leaves room.
 */
@Composable
fun AddButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    contentDescription: String = stringResource(R.string.kit_new_item),
) {
    val colors = HavenTheme.colors
    Box(
        modifier = modifier
            .size(AddSize)
            .shadow(elevation = 10.dp, shape = CircleShape)
            .clip(CircleShape)
            .havenClickable(onClick = onClick)
            .background(colors.primary)
            .semantics { this.contentDescription = contentDescription },
        contentAlignment = Alignment.Center,
    ) {
        IconGlyph(HavenIcon.Plus, contentDescription = null, tint = colors.onPrimary, size = 26.dp)
    }
}

@PreviewLightDark
@Composable
private fun AddButtonPreview() {
    KitPreview { AddButton(onClick = {}) }
}
