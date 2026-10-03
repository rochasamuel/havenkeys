package net.havenkeys.android.ui.components

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The bar of a full-screen screen over the shell (item, editor, generator,
 * devices, autofill setup): Back, then the offline marker, Lock and the
 * screen's own [actions]. No title: the screen's large title sits under it.
 * The offline marker is in a polite live region, as the shell's bar has it.
 */
@Composable
fun ScreenBar(
    onBack: () -> Unit,
    online: Boolean,
    onLock: () -> Unit,
    modifier: Modifier = Modifier,
    actions: @Composable RowScope.() -> Unit = {},
) {
    Row(
        modifier.fillMaxWidth().padding(horizontal = 4.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        HavenIconButton(HavenIcon.ChevronLeft, stringResource(R.string.item_back), onClick = onBack)
        Spacer(Modifier.weight(1f))
        Box(Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
            if (!online) Pill(stringResource(R.string.vault_offline), Modifier.padding(end = 4.dp))
        }
        HavenIconButton(HavenIcon.Lock, stringResource(R.string.vault_lock_now), onClick = onLock)
        actions()
    }
}

/** Why a screen cannot save now: the offline glyph and a muted line (brass never fills a surface). */
@Composable
fun OfflineNote(text: String, modifier: Modifier = Modifier) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().padding(vertical = 8.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconGlyph(HavenIcon.CloudOff, contentDescription = null, tint = colors.muted, size = 18.dp)
        HavenText(text, style = HavenTheme.type.body, color = colors.muted)
    }
}
