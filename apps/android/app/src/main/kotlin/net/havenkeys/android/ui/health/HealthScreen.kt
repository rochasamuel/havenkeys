package net.havenkeys.android.ui.health

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import androidx.core.net.toUri
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.ScreenBar
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenIconButton
import net.havenkeys.android.ui.kit.HavenMenu
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.MenuItem
import net.havenkeys.android.ui.kit.SectionAction
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.kit.SegmentedControl
import net.havenkeys.android.ui.shell.EmptyLine
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.LargeTitle
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.HealthKind

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/** Where the Health screen leads. Each takes an item's id, never a value. */
class HealthNavigation(
    val onOpen: (String) -> Unit,
    val onEdit: (String) -> Unit,
    val onBack: () -> Unit,
    val onLock: () -> Unit,
)

/**
 * Vault health (spec §8): the seven checks with their counts and why each
 * matters, then the logins that fail them. Everything shown is an overview
 * (title, username) and check kinds; nothing is revealed. Dismiss, Undo and
 * Change password write to the server, so they wait for it.
 */
@Composable
fun HealthScreen(
    viewModel: HealthViewModel,
    online: Boolean,
    navigation: HealthNavigation,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    LaunchedEffect(viewModel) { viewModel.shown() }

    HavenScaffold(
        modifier = modifier,
        topBar = { ScreenBar(onBack = navigation.onBack, online = online, onLock = navigation.onLock) },
    ) { padding ->
        LazyColumn(
            Modifier.fillMaxSize(),
            contentPadding = PaddingValues(bottom = padding.calculateBottomPadding() + HavenSpacing.gutter),
        ) {
            item(key = "title") { LargeTitle(stringResource(R.string.health_title), Gutter) }
            item(key = "intro") {
                HavenText(
                    stringResource(R.string.health_intro),
                    Gutter.padding(bottom = 8.dp),
                    color = HavenTheme.colors.muted,
                )
            }
            state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
            report(state, online, viewModel, navigation)
        }
    }
}

/** The report under the intro: the loading line, or the checks, the filters and the logins. */
private fun LazyListScope.report(
    state: HealthUiState,
    online: Boolean,
    viewModel: HealthViewModel,
    navigation: HealthNavigation,
) {
    val counts = state.counts
    if (counts == null) {
        if (state.loading) {
            item(key = "loading") {
                EmptyLine(
                    stringResource(R.string.health_loading),
                    Gutter.semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
        }
        return
    }
    item(key = "checks-title") {
        SectionHeader(stringResource(R.string.health_checks), Gutter.padding(top = 12.dp))
    }
    insetGroup(KIND_ORDER, key = { "check-$it" }) { kind ->
        val selected = (state.filter as? HealthFilter.Kind)?.kind == kind
        KindRow(kind, counts.of(kind), selected) {
            viewModel.filter(if (selected) HealthFilter.All else HealthFilter.Kind(kind))
        }
    }
    item(key = "filters") {
        SegmentedControl(
            options = listOf(stringResource(R.string.health_all), stringResource(R.string.health_dismissed)),
            selectedIndex = if (state.filter == HealthFilter.Dismissed) 1 else 0,
            onSelect = { viewModel.filter(if (it == 1) HealthFilter.Dismissed else HealthFilter.All) },
            modifier = Gutter.padding(top = 24.dp),
        )
    }
    item(key = "issues-title") { IssuesHeader(state.filter, viewModel::filter) }
    if (state.rows.isEmpty()) {
        item(key = "empty") {
            val nothing = state.filter == HealthFilter.All && state.total == 0
            EmptyLine(stringResource(if (nothing) R.string.health_empty else R.string.health_empty_filtered), Gutter)
        }
    }
    insetGroup(state.rows, key = { "${it.id}-${it.dismissed}" }) { row ->
        IssueRow(row, state.filter, online, viewModel, navigation)
    }
}

/**
 * One check: its name, why it matters, and how many logins fail it. Tapping
 * shows only those. A check nobody fails steps back (muted name and count),
 * so the eye lands on the ones that need work.
 */
@Composable
private fun KindRow(kind: HealthKind, count: Int, selected: Boolean, onClick: () -> Unit) {
    val colors = HavenTheme.colors
    val ink = if (count > 0) colors.textStrong else colors.muted
    GroupRow(
        Modifier
            .background(if (selected) colors.sel else Color.Transparent)
            .semantics { this.selected = selected },
        onClick = onClick,
        trailing = {
            // TrailingText's place on the 16dp margin, in the title's ink.
            HavenText(count.toString(), Modifier.padding(end = 8.dp), style = HavenTheme.type.value, color = ink)
        },
    ) {
        HavenText(stringResource(kind.titleRes()), style = HavenTheme.type.value, color = ink)
        HavenText(stringResource(kind.bodyRes()), style = HavenTheme.type.rowSubtitle, color = colors.muted)
    }
}

/**
 * Under one check, the list is headed by that check's name with a way back
 * to every issue. Under All issues and Dismissed the segmented control above
 * already names the list, so it only keeps its distance.
 */
@Composable
private fun IssuesHeader(filter: HealthFilter, onFilter: (HealthFilter) -> Unit) {
    if (filter !is HealthFilter.Kind) {
        Spacer(Modifier.height(12.dp))
        return
    }
    SectionHeader(
        stringResource(filter.kind.titleRes()),
        Gutter.padding(top = 12.dp),
        action = SectionAction(stringResource(R.string.health_all)) { onFilter(HealthFilter.All) },
    )
}

/** What one row offers, decided from its checks and the filter. */
private class RowOffer(row: HealthRow, filter: HealthFilter) {
    /** Under one check's filter, the row speaks of that check alone. */
    val single: HealthKind? = (filter as? HealthFilter.Kind)?.kind
    val kinds: List<HealthKind> = single?.let(::listOf) ?: KIND_ORDER.filter { it in row.kinds }
    val passwordFix: Boolean =
        if (single != null) single in PASSWORD_KINDS else !row.dismissed && row.kinds.any { it in PASSWORD_KINDS }
    val helpKind: HealthKind? = when {
        single != null -> single.takeIf { it in HELP_KINDS }
        row.dismissed -> null
        else -> kinds.firstOrNull { it in HELP_KINDS }
    }
}

/**
 * One login: its title and username, the checks it fails as chips, and its
 * actions behind More. Tapping the row opens the login. While a Dismiss or
 * Undo waits for the next report, More is disabled, so a second change cannot
 * be built on a report from before the first.
 */
@Composable
private fun IssueRow(
    row: HealthRow,
    filter: HealthFilter,
    online: Boolean,
    viewModel: HealthViewModel,
    navigation: HealthNavigation,
) {
    val offer = RowOffer(row, filter)
    // Rust's link, asked once per row; "How to enable" shows only when there is one.
    val helpUrl by produceState<String?>(null, row.id, offer.helpKind) {
        value = offer.helpKind?.let { viewModel.helpUrl(row.id, it) }
    }
    var open by remember { mutableStateOf(false) }
    val items = rowActions(row, offer, online, helpUrl, viewModel, navigation)
    GroupRow(
        onClick = { navigation.onOpen(row.id) },
        onClickLabel = stringResource(R.string.health_open),
        trailing = {
            Box {
                HavenIconButton(
                    HavenIcon.More,
                    stringResource(R.string.health_row_actions, row.title),
                    onClick = { open = true },
                    enabled = !row.busy,
                )
                HavenMenu(expanded = open && !row.busy, onDismiss = { open = false }, items = items)
            }
        },
    ) {
        GroupRowText(row.title, row.subtitle)
        HealthChips(offer.kinds, row.groupSize, row.duplicates, Modifier.padding(top = 6.dp))
    }
}

/** Open; then, online, Change password; How to enable with Rust's link; Dismiss or Undo per check. */
@Composable
@Suppress("LongParameterList") // The row, what it offers, and the three things its entries act through.
private fun rowActions(
    row: HealthRow,
    offer: RowOffer,
    online: Boolean,
    helpUrl: String?,
    viewModel: HealthViewModel,
    navigation: HealthNavigation,
): List<MenuItem> {
    val context = LocalContext.current
    val open = MenuItem(stringResource(R.string.health_open), { navigation.onOpen(row.id) })
    val change = MenuItem(
        stringResource(R.string.health_change_password),
        { navigation.onEdit(row.id) },
        HavenIcon.Edit,
    )
    val help = helpUrl?.let { url ->
        MenuItem(stringResource(R.string.health_how_to_enable), { openLink(context, url) }, HavenIcon.Globe)
    }
    val single = offer.single
    val changes = if (single != null) {
        listOf(MenuItem(stringResource(R.string.health_dismiss), { viewModel.dismiss(row.id, single) }))
    } else {
        offer.kinds.map { kind ->
            val chip = chipLabel(kind, row.groupSize, row.duplicates)
            if (row.dismissed) {
                MenuItem(stringResource(R.string.health_undo_named, chip), { viewModel.undo(row.id, kind) })
            } else {
                MenuItem(stringResource(R.string.health_dismiss_named, chip), { viewModel.dismiss(row.id, kind) })
            }
        }
    }
    val writes = if (online) changes else emptyList()
    return listOfNotNull(open, change.takeIf { online && offer.passwordFix }, help) + writes
}

/** Opens Rust's link exactly as given; a phone with no browser simply does nothing. */
private fun openLink(context: Context, url: String) {
    try {
        context.startActivity(Intent(Intent.ACTION_VIEW, url.toUri()))
    } catch (@Suppress("SwallowedException") e: ActivityNotFoundException) {
        // Nothing to open it with.
    }
}
