package net.havenkeys.android.ui.search

import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.text.input.TextFieldState
import androidx.compose.foundation.text.input.setTextAndPlaceCursorAtEnd
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.kit.ButtonStyle
import net.havenkeys.android.ui.kit.GroupRow
import net.havenkeys.android.ui.kit.GroupRowText
import net.havenkeys.android.ui.kit.HavenButton
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.kit.HavenText
import net.havenkeys.android.ui.kit.SectionAction
import net.havenkeys.android.ui.kit.SectionHeader
import net.havenkeys.android.ui.shell.ErrorLine
import net.havenkeys.android.ui.shell.NoSharedTitle
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.Origins
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.SummaryRow
import net.havenkeys.android.ui.shell.insetGroup
import net.havenkeys.android.ui.theme.HavenSpacing
import net.havenkeys.android.ui.theme.HavenTheme

/** Recent searches wait this long, so they fade in after the pill has grown (spec §7). */
private const val RECENTS_DELAY_MILLIS = 160

private val Gutter = Modifier.padding(horizontal = HavenSpacing.gutter)

/**
 * Search (spec §6.3): the pill grown into a focused field. Empty, it shows
 * recent searches with Clear; typing shows live results; opening one
 * records the query. The field's text is `remember`ed, never saved, and
 * the ViewModel keeps the query in memory only: a query never goes into a
 * route, saved instance state or a log.
 */
@Composable
fun SearchScreen(
    viewModel: SearchViewModel,
    onOpen: OpenItem,
    onCancel: () -> Unit,
    modifier: Modifier = Modifier,
    fieldModifier: Modifier = Modifier,
    sharedTitle: SharedTitle = NoSharedTitle,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val field = remember(viewModel) { TextFieldState(viewModel.state.value.query) }
    val focus = remember { FocusRequester() }
    // The field owns its text: typing flows to the ViewModel and is never pushed back.
    LaunchedEffect(field) { snapshotFlow { field.text.toString() }.collect(viewModel::setQuery) }
    // Only a change the ViewModel itself made (a recent search tapped, the lock wipe) goes into the field.
    val seen = remember(viewModel) { intArrayOf(viewModel.state.value.queryRevision) }
    LaunchedEffect(state.queryRevision) {
        if (seen[0] != state.queryRevision) {
            seen[0] = state.queryRevision
            field.setTextAndPlaceCursorAtEnd(state.query)
        }
    }
    LaunchedEffect(focus) { focus.requestFocus() }
    val open: OpenItem = remember(viewModel, onOpen) {
        { id, origin ->
            viewModel.opened()
            onOpen(id, origin)
        }
    }
    Column(
        modifier
            .fillMaxSize()
            .background(HavenTheme.colors.pane)
            .statusBarsPadding()
            .navigationBarsPadding()
            .imePadding(),
    ) {
        Row(
            Modifier.fillMaxWidth().padding(start = HavenSpacing.gutter, end = 4.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            SearchField(field, Modifier.weight(1f).then(fieldModifier).focusRequester(focus))
            HavenButton(stringResource(R.string.search_cancel), onClick = onCancel, style = ButtonStyle.Quiet)
        }
        LazyColumn(Modifier.fillMaxSize(), contentPadding = PaddingValues(bottom = HavenSpacing.gutter)) {
            if (state.query.isBlank()) {
                recents(state.recents, viewModel::useRecent, viewModel::clearRecents)
            } else {
                results(state, open, sharedTitle)
            }
        }
    }
}

private fun LazyListScope.recents(recents: List<String>, onUse: (String) -> Unit, onClear: () -> Unit) {
    if (recents.isEmpty()) return
    item(key = "recents-title") {
        Later {
            SectionHeader(
                stringResource(R.string.search_recent),
                Gutter.padding(top = 8.dp),
                action = SectionAction(stringResource(R.string.search_clear), onClear),
            )
        }
    }
    insetGroup(recents, key = null, around = { Later(it) }) { query ->
        GroupRow(onClick = { onUse(query) }, icon = HavenIcon.Clock) { GroupRowText(query) }
    }
}

private fun LazyListScope.results(state: SearchUiState, open: OpenItem, sharedTitle: SharedTitle) {
    state.errorCode?.let { code -> item(key = "error") { ErrorLine(code, Gutter) } }
    if (state.searched && state.results.isEmpty() && state.errorCode == null) {
        item(key = "none") {
            Column(Gutter.padding(vertical = 16.dp)) {
                HavenText(
                    stringResource(R.string.vault_no_matches),
                    style = HavenTheme.type.titleSmall,
                    color = HavenTheme.colors.textStrong,
                )
                HavenText(stringResource(R.string.vault_search_hint), color = HavenTheme.colors.muted)
            }
        }
    }
    insetGroup(state.results, key = { it.id }) { summary ->
        SummaryRow(summary, Origins.SEARCH, open, sharedTitle = sharedTitle)
    }
}

/** Fades its content in after the pill has grown; under "Remove animations" it is simply there. */
@Composable
private fun Later(content: @Composable () -> Unit) {
    val motion = HavenTheme.motion
    val shown = remember { Animatable(if (motion.reduced) 1f else 0f) }
    LaunchedEffect(shown) { shown.animateTo(1f, motion.fadeSpec(delayMillis = RECENTS_DELAY_MILLIS)) }
    Box(Modifier.graphicsLayer { alpha = shown.value }) { content() }
}
