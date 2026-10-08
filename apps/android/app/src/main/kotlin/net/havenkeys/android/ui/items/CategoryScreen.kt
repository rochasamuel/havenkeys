package net.havenkeys.android.ui.items

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.edit.tagKey
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
import uniffi.havenkeys_mobile.ItemSummary

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
) = FilteredList(
    viewModel,
    stringResource(category.label),
    filter = category,
    category::keeps,
    onOpen,
    contentPadding,
    modifier,
    sharedTitle,
)

/**
 * One tag's items, A–Z: a category list whose title is the tag. When no
 * item carries the tag any more, [onGone] takes the user back to Items, as
 * the desktop falls back to All items.
 */
@Suppress("LongParameterList") // the category list's, plus the way out
@Composable
fun TagScreen(
    viewModel: ItemListViewModel,
    tag: String,
    onOpen: OpenItem,
    onGone: () -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val gone = tagGone(state, tag)
    LaunchedEffect(gone) { if (gone) onGone() }
    FilteredList(
        viewModel,
        tag,
        filter = "tag:${tagKey(tag)}",
        { carries(it, tag) },
        onOpen,
        contentPadding,
        modifier,
        sharedTitle,
    )
}

@Suppress("LongParameterList") // the two lists' shared body
@Composable
private fun FilteredList(
    viewModel: ItemListViewModel,
    title: String,
    /** Which list this is (a category, or "tag:" and its name): the filter's identity. */
    filter: Any,
    keeps: (ItemSummary) -> Boolean,
    onOpen: OpenItem,
    contentPadding: PaddingValues,
    modifier: Modifier,
    sharedTitle: SharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val shown = remember(state.items, filter) { state.items.filter(keeps) }
    PullToRefresh(
        refreshing = state.refreshing,
        onRefresh = viewModel::refresh,
        modifier = modifier.fillMaxSize().background(HavenTheme.colors.pane),
    ) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") { LargeTitle(title, Gutter.padding(top = 8.dp)) }
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
