package net.havenkeys.android.ui.home

import androidx.annotation.StringRes
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.InsetGroup
import net.havenkeys.android.ui.kit.PullToRefresh
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.Settle
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import uniffi.havenkeys_mobile.ItemSummary

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * Home (spec §6.5): the identity on top, then Recently added and Frequently
 * used. Pull to refresh syncs. The lists reload each time Home shows. The
 * first time a Home has its data, its groups settle in sequence (spec §7);
 * nothing is composed before then, so nothing settles early or twice.
 */
@Composable
fun HomeScreen(
    viewModel: HomeViewModel,
    onOpen: OpenItem,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LifecycleResumeEffect(viewModel) {
        viewModel.shown()
        onPauseOrDispose {}
    }
    // Rows composed in the frame the data arrives settle; rows scrolled in later are just there.
    var settle by remember { mutableStateOf(!viewModel.settled) }
    LaunchedEffect(viewModel, state.loading) {
        if (!state.loading) {
            viewModel.settled = true
            settle = false
        }
    }
    val rows = HomeRows(onOpen, sharedTitle, settle)
    PullToRefresh(refreshing = state.refreshing, onRefresh = viewModel::refresh, modifier = modifier.fillMaxSize()) {
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(
                top = 8.dp,
                bottom = contentPadding.calculateBottomPadding() + HavenSpacing.gutter,
            ),
        ) {
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
            if (!state.loading) {
                state.identity?.let { card ->
                    item(key = "identity") { Settle(0, active = rows.settle) { IdentityCardRow(card, onOpen) } }
                }
                activityGroup(rows, Recent, state.recent)
                activityGroup(rows, Frequent, state.frequent)
            }
        }
    }
}

/** What every Home row needs besides its item. */
private class HomeRows(val onOpen: OpenItem, val sharedTitle: SharedTitle, val settle: Boolean)

/** One of Home's two lists: its title, what it says when empty, its rows' origin, its place in the settle. */
private class GroupSpec(@StringRes val title: Int, @StringRes val emptyText: Int, val origin: String, val index: Int)

private val Recent = GroupSpec(R.string.home_recent, R.string.vault_empty_hint, Origins.RECENT, index = 1)
private val Frequent = GroupSpec(R.string.home_frequent, R.string.home_frequent_empty, Origins.FREQUENT, index = 2)

/** A titled group of item rows; keys carry the origin, since one item can be in both groups. */
private fun LazyListScope.activityGroup(rows: HomeRows, spec: GroupSpec, items: List<ItemSummary>) {
    item(key = "${spec.origin}-title") {
        Settle(spec.index, active = rows.settle) {
            SectionHeader(stringResource(spec.title), Gutter.padding(top = 12.dp))
        }
    }
    if (items.isEmpty()) {
        item(key = "${spec.origin}-empty") {
            Settle(spec.index, active = rows.settle) { EmptyLine(stringResource(spec.emptyText), Gutter) }
        }
    }
    insetGroup(
        items,
        key = { "${spec.origin}-${it.id}" },
        around = { Settle(spec.index, active = rows.settle, content = it) },
    ) { summary ->
        SummaryRow(summary, spec.origin, rows.onOpen, sharedTitle = rows.sharedTitle)
    }
}

/** The identity: its title and the kinds of detail it holds, or an invitation to add them. */
@Composable
private fun IdentityCardRow(card: IdentityCard, onOpen: OpenItem) {
    val summary = if (card.parts.isEmpty()) {
        stringResource(R.string.home_identity_empty)
    } else {
        card.parts.map { stringResource(it.label) }.joinToString(" · ")
    }
    InsetGroup(Gutter) {
        row {
            GroupRow(onClick = { onOpen(card.id, Origins.IDENTITY) }, icon = HavenIcon.IdCard) {
                GroupRowText(card.title, summary)
            }
        }
    }
}
