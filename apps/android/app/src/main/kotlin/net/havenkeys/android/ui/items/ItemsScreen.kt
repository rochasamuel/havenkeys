package net.havenkeys.android.ui.items

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.TrailingText
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * The Items tab (spec §6.7): each category with its count, then each tag in
 * use with its count; each opens its own list.
 */
@Composable
fun ItemsScreen(
    viewModel: ItemListViewModel,
    onCategory: (Category) -> Unit,
    onTag: (String) -> Unit,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val counts = remember(state.items) { categoryCounts(state.items) }
    val tags = remember(state.items) { tagCounts(state.items) }
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        Column(
            Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = HavenSpacing.gutter)
                .padding(top = 8.dp, bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            LargeTitle(stringResource(R.string.tab_items))
            state.errorCode?.let { ErrorLine(it) }
            InsetGroup {
                Category.entries.forEach { category ->
                    row {
                        GroupRow(
                            onClick = { onCategory(category) },
                            icon = category.icon,
                            trailing = { if (!state.loading) TrailingText(counts.getValue(category).toString()) },
                            chevron = true,
                        ) { GroupRowText(stringResource(category.label)) }
                    }
                }
            }
            if (tags.isNotEmpty()) {
                SectionHeader(stringResource(R.string.items_tags), Modifier.padding(top = 16.dp))
                InsetGroup {
                    tags.forEach { (name, count) ->
                        row {
                            GroupRow(
                                onClick = { onTag(name) },
                                icon = HavenIcon.Tag,
                                iconTint = HavenTheme.colors.brassInk,
                                trailing = { TrailingText(count.toString()) },
                                chevron = true,
                            ) { GroupRowText(name) }
                        }
                    }
                }
            }
        }
    }
}
