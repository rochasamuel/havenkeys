package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.DISABLED_ALPHA
import net.havenkeys.android.ui.kit.HavenSheet
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.havenClickable
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenTheme

private val TileHeight = 96.dp

/**
 * The add button's sheet (spec §6.6): Login, Secure note, Card, Generate
 * password, as tiles that settle in one after another. Offline the item
 * tiles are dimmed and say why; the generator stays. A pick closes the
 * sheet at once and goes straight to its screen; dragging down, the
 * backdrop and Back close it on its spring.
 */
@Composable
fun AddSheet(tiles: List<AddTileState>, onPick: (AddTile) -> Unit, onDismiss: () -> Unit) {
    HavenSheet(onDismiss = onDismiss, title = stringResource(R.string.kit_new_item)) {
        AddTiles(tiles, onPick)
    }
}

/** The tiles without the sheet's window; the screenshots draw them inline. */
@Composable
internal fun AddTiles(tiles: List<AddTileState>, onPick: (AddTile) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        tiles.chunked(2).forEachIndexed { rowIndex, pair ->
            Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                pair.forEachIndexed { columnIndex, state ->
                    Settle(index = rowIndex * 2 + columnIndex, modifier = Modifier.weight(1f)) {
                        AddTileButton(state) { onPick(state.tile) }
                    }
                }
            }
        }
        if (tiles.any { !it.enabled }) {
            HavenText(stringResource(R.string.add_offline), color = HavenTheme.colors.muted)
        }
    }
}

@Composable
private fun AddTileButton(state: AddTileState, onClick: () -> Unit) {
    val colors = HavenTheme.colors
    Column(
        Modifier
            .fillMaxWidth()
            .heightIn(min = TileHeight)
            .alpha(if (state.enabled) 1f else DISABLED_ALPHA)
            .clip(HavenShape.group)
            .havenClickable(enabled = state.enabled, onClick = onClick)
            .background(colors.group)
            .border(1.dp, colors.groupLine, HavenShape.group)
            .padding(14.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        IconGlyph(state.tile.icon, contentDescription = null, tint = colors.brassInk, size = 24.dp)
        HavenText(stringResource(state.tile.label), style = HavenTheme.type.rowTitle, color = colors.textStrong)
    }
}
