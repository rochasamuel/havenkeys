@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.shell

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.IconGlyph
import net.havenkeys.android.ui.kit.Pill
import net.havenkeys.android.ui.kit.ProgressRing
import net.havenkeys.android.ui.kit.havenClickable
import net.havenkeys.android.ui.theme.HavenShape
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** What the top bar's controls do; built once by the shell, so the bar skips recomposition. */
class TopBarActions(val onSearch: () -> Unit, val onSync: () -> Unit, val onLock: () -> Unit)

/**
 * The top bar of every tab and category list (spec §6.2): the search pill,
 * the offline badge when offline, Sync now (a ring while syncing) and Lock.
 * Sync now is the visible, TalkBack-reachable twin of pull to refresh. The
 * bar sits outside the tabs' NavHost and takes only stable values, so a tab
 * change neither moves nor recomposes it. [pillModifier] carries the pill's
 * shared bounds into search.
 */
@Composable
fun ShellTopBar(
    online: Boolean,
    syncing: Boolean,
    actions: TopBarActions,
    modifier: Modifier = Modifier,
    pillModifier: Modifier = Modifier,
) {
    val colors = HavenTheme.colors
    Row(
        modifier.fillMaxWidth().padding(start = HavenSpacing.gutter, end = 4.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(
            Modifier
                .weight(1f)
                .then(pillModifier)
                .heightIn(min = HavenSpacing.touch)
                .clip(HavenShape.pill)
                .havenClickable(onClick = actions.onSearch)
                .background(colors.field)
                .padding(horizontal = 14.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconGlyph(HavenIcon.Search, contentDescription = null, tint = colors.muted, size = 18.dp)
            Spacer(Modifier.width(10.dp))
            HavenText(stringResource(R.string.shell_search), style = HavenTheme.type.value, color = colors.muted)
        }
        if (!online) Pill(stringResource(R.string.shell_offline), Modifier.padding(start = 8.dp))
        if (syncing) {
            Box(Modifier.size(HavenSpacing.touch), contentAlignment = Alignment.Center) {
                ProgressRing(progress = null, size = 20.dp, contentDescription = stringResource(R.string.shell_syncing))
            }
        } else {
            HavenIconButton(HavenIcon.Refresh, stringResource(R.string.shell_sync), onClick = actions.onSync)
        }
        HavenIconButton(HavenIcon.Lock, stringResource(R.string.vault_lock_now), onClick = actions.onLock)
    }
}
