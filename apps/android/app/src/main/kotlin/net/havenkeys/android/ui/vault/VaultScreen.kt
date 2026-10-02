package net.havenkeys.android.ui.vault

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Add
import androidx.compose.material.icons.outlined.CloudOff
import androidx.compose.material.icons.outlined.MoreVert
import androidx.compose.material.icons.outlined.Password
import androidx.compose.material.icons.outlined.Search
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExtendedFloatingActionButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.disabled
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.components.ItemRow
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.theme.HavenTheme
import uniffi.havenkeys_mobile.ItemKind

private val filterLabels = listOf(
    Filter.ALL to R.string.vault_filter_all,
    Filter.LOGINS to R.string.vault_filter_logins,
    Filter.NOTES to R.string.vault_filter_notes,
    Filter.CARDS to R.string.vault_filter_cards,
    Filter.IDENTITIES to R.string.vault_filter_identities,
    Filter.PASSKEYS to R.string.vault_filter_passkeys,
)

/** Where the list leads; a route carries at most an item's id or a kind. */
class VaultNavigation(
    val onOpen: (id: String) -> Unit,
    val onNew: (ItemKind) -> Unit,
    val onLock: () -> Unit,
    val onGenerator: () -> Unit,
    val onSettings: () -> Unit,
)

/**
 * The vault list: search, a filter by kind, pull to refresh from the server.
 * [titleModifier] gives each row's title the shared element it carries to
 * the item's screen.
 */
@Composable
fun VaultScreen(
    viewModel: VaultViewModel,
    navigation: VaultNavigation,
    modifier: Modifier = Modifier,
    titleModifier: @Composable (id: String) -> Modifier = { Modifier },
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    Scaffold(
        topBar = { VaultTopBar(state.online, navigation.onLock, navigation.onGenerator, navigation.onSettings) },
        floatingActionButton = { NewItemButton(state.online, navigation.onNew) },
        containerColor = MaterialTheme.colorScheme.background,
        modifier = modifier,
    ) { padding ->
        Column(Modifier.padding(padding).fillMaxSize()) {
            if (!state.online) OfflineBanner()
            SearchAndFilter(state, viewModel)
            state.errorCode?.let { code ->
                Text(
                    text = stringResource(errorText(code)),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                )
            }
            PullToRefreshBox(
                isRefreshing = state.refreshing,
                onRefresh = viewModel::refresh,
                modifier = Modifier.fillMaxSize(),
            ) {
                LazyColumn(Modifier.fillMaxSize()) {
                    if (state.items.isEmpty() && !state.loading && state.errorCode == null) {
                        item { EmptyList(searching = state.query.isNotBlank()) }
                    }
                    items(state.items, key = { it.id }) { summary ->
                        ItemRow(
                            summary = summary,
                            onClick = { navigation.onOpen(summary.id) },
                            titleModifier = titleModifier(summary.id),
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun SearchAndFilter(state: VaultUiState, viewModel: VaultViewModel) {
    OutlinedTextField(
        value = state.query,
        onValueChange = viewModel::setQuery,
        placeholder = { Text(stringResource(R.string.vault_search)) },
        leadingIcon = { Icon(Icons.Outlined.Search, contentDescription = null) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, imeAction = ImeAction.Search),
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp),
    )
    Row(
        modifier = Modifier.horizontalScroll(rememberScrollState()).padding(horizontal = 16.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        filterLabels.forEach { (filter, label) ->
            FilterChip(
                selected = state.filter == filter,
                onClick = { viewModel.setFilter(filter) },
                label = { Text(stringResource(label)) },
            )
        }
    }
}

@Composable
private fun VaultTopBar(online: Boolean, onLock: () -> Unit, onGenerator: () -> Unit, onSettings: () -> Unit) {
    HavenTopBar(
        title = stringResource(R.string.vault_title),
        online = online,
        onLock = onLock,
        actions = { VaultMenu(onGenerator, onSettings) },
    )
}

@Composable
private fun VaultMenu(onGenerator: () -> Unit, onSettings: () -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { open = true }) {
            Icon(Icons.Outlined.MoreVert, contentDescription = stringResource(R.string.vault_more))
        }
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.generator_title)) },
                leadingIcon = { Icon(Icons.Outlined.Password, contentDescription = null) },
                onClick = {
                    open = false
                    onGenerator()
                },
            )
            DropdownMenuItem(
                text = { Text(stringResource(R.string.settings_title)) },
                leadingIcon = { Icon(Icons.Outlined.Settings, contentDescription = null) },
                onClick = {
                    open = false
                    onSettings()
                },
            )
        }
    }
}

private val newKinds = listOf(
    ItemKind.LOGIN to R.string.vault_new_login,
    ItemKind.SECURE_NOTE to R.string.vault_new_note,
    ItemKind.CARD to R.string.vault_new_card,
)

/** Creating needs the server: offline the button is greyed, under the offline banner. */
@Composable
private fun NewItemButton(online: Boolean, onNew: (ItemKind) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        ExtendedFloatingActionButton(
            text = { Text(stringResource(R.string.vault_new_item)) },
            icon = { Icon(Icons.Outlined.Add, contentDescription = null) },
            onClick = { if (online) open = true },
            containerColor = if (online) {
                MaterialTheme.colorScheme.primaryContainer
            } else {
                MaterialTheme.colorScheme.surfaceVariant
            },
            contentColor = if (online) {
                MaterialTheme.colorScheme.onPrimaryContainer
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = DISABLED_ALPHA)
            },
            modifier = Modifier.semantics { if (!online) disabled() },
        )
        DropdownMenu(expanded = open && online, onDismissRequest = { open = false }) {
            newKinds.forEach { (kind, label) ->
                DropdownMenuItem(
                    text = { Text(stringResource(label)) },
                    onClick = {
                        open = false
                        onNew(kind)
                    },
                )
            }
        }
    }
}

private const val DISABLED_ALPHA = 0.38f

@Composable
private fun OfflineBanner() {
    Surface(color = HavenTheme.colors.brassSoft, modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Outlined.CloudOff, contentDescription = null, tint = HavenTheme.colors.brassInk)
            Text(stringResource(R.string.vault_offline_banner), style = MaterialTheme.typography.bodyMedium)
        }
    }
}

@Composable
private fun EmptyList(searching: Boolean) {
    Column(Modifier.fillMaxWidth().padding(24.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Text(
            text = stringResource(if (searching) R.string.vault_no_matches else R.string.vault_empty),
            style = MaterialTheme.typography.titleMedium,
            color = HavenTheme.colors.textStrong,
        )
        Text(
            text = stringResource(if (searching) R.string.vault_search_hint else R.string.vault_empty_hint),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}
