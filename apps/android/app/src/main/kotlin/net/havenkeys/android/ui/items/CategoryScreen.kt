package net.havenkeys.android.ui.items

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
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
import net.havenkeys.android.ui.theme.HavenTheme

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * One category's items, A–Z, inside the shell (spec §6.7): a large title
 * under the shared top bar. No back chevron: the system Back returns to the
 * categories. The list has its own ground, so the categories it slides over
 * never show through it. Pull to refresh syncs.
 */
@Composable
fun CategoryScreen(
    viewModel: ItemListViewModel,
    category: Category,
    onOpen: OpenItem,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val shown = remember(state.items, category) { state.items.filter(category::keeps) }
    PullToRefresh(
        refreshing = state.refreshing,
        onRefresh = viewModel::refresh,
        modifier = modifier.fillMaxSize().background(HavenTheme.colors.pane),
    ) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") { LargeTitle(stringResource(category.label), Gutter.padding(top = 8.dp)) }
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
