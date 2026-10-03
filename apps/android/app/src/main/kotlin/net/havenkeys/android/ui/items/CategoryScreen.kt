package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/** How far a 22dp glyph sits inside its 48dp target. */
private val BackInset = 13.dp

/**
 * One category's items, A–Z, inside the shell (spec §6.7): a large title
 * under the shared top bar, and a back chevron because that bar has none.
 * Pull to refresh syncs.
 */
@Composable
fun CategoryScreen(
    viewModel: ItemListViewModel,
    category: Category,
    onOpen: OpenItem,
    onBack: () -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val shown = remember(state.items, category) { state.items.filter(category::keeps) }
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") {
                Column(Gutter) {
                    // Pulled out by the target's inset, so the chevron's ink lines up with the title.
                    HavenIconButton(
                        HavenIcon.ChevronLeft,
                        stringResource(R.string.item_back),
                        onClick = onBack,
                        modifier = Modifier.offset(x = -BackInset),
                    )
                    LargeTitle(stringResource(category.label))
                }
            }
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
            if (shown.isEmpty() && !state.loading && state.errorCode == null) {
                item(key = "empty") { EmptyLine(stringResource(R.string.items_empty), Gutter) }
            }
            insetGroup(shown, key = { it.id }) { summary ->
                SummaryRow(summary, Origins.CATEGORY, onOpen, sharedTitle = sharedTitle)
            }
        }
    }
}
