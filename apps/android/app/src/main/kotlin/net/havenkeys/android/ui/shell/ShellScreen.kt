@file:Suppress("MatchingDeclarationName")

package net.havenkeys.android.ui.shell

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import net.havenkeys.android.ui.components.errorText
import net.havenkeys.android.ui.kit.AddButton
import net.havenkeys.android.ui.kit.HavenScaffold
import net.havenkeys.android.ui.kit.ToastTone
import net.havenkeys.android.ui.kit.rememberToastState
import uniffi.havenkeys_mobile.ItemKind

/** Where the shell leads outside itself; a route carries at most a kind. */
class ShellNavigation(val onSearch: () -> Unit, val onNew: (ItemKind) -> Unit, val onGenerator: () -> Unit)

/**
 * The shell (spec §6.1): the fixed top bar, the current tab's content with
 * its own back stack, the bottom bar, and the add button on Home and Items.
 * The bars live outside the tabs' NavHost, so a tab change leaves them
 * still. The lock replaces this whole destination with Unlock.
 */
@Composable
fun ShellScreen(
    viewModel: ShellViewModel,
    screens: ShellScreens,
    navigation: ShellNavigation,
    modifier: Modifier = Modifier,
    searchPillModifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val tabs = rememberNavController()
    val entry by tabs.currentBackStackEntryAsState()
    val tab = entry?.destination?.tab() ?: Tab.HOME
    var adding by remember { mutableStateOf(false) }
    val toasts = rememberToastState()
    val actions = remember(viewModel, navigation) {
        TopBarActions(onSearch = navigation.onSearch, onSync = viewModel::sync, onLock = viewModel::lock)
    }
    val syncFailed = state.syncError?.let { stringResource(errorText(it)) }
    LaunchedEffect(syncFailed) {
        if (syncFailed != null) {
            toasts.show(syncFailed, ToastTone.Alert)
            viewModel.syncErrorShown()
        }
    }
    // The add button floats on Home and Items (category lists included), not on Settings.
    val addButton: (@Composable () -> Unit)? = if (tab == Tab.SETTINGS) null else {
        { AddButton(onClick = { adding = true }) }
    }
    HavenScaffold(
        modifier = modifier,
        topBar = { ShellTopBar(state.online, state.syncing, actions, pillModifier = searchPillModifier) },
        bottomBar = { BottomBar(tab, onSelect = tabs::selectTab) },
        floatingButton = addButton,
        toastState = toasts,
    ) { padding ->
        ShellNavHost(tabs, screens, padding)
    }
    if (adding) {
        AddSheet(
            state.addTiles,
            onPick = { tile ->
                adding = false
                val kind = tile.kind
                if (kind == null) navigation.onGenerator() else navigation.onNew(kind)
            },
            onDismiss = { adding = false },
        )
    }
}
