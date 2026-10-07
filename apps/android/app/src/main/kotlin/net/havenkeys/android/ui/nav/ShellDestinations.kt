package net.havenkeys.android.ui.nav

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavHostController
import net.havenkeys.android.ui.home.HomeScreen
import net.havenkeys.android.ui.home.HomeViewModel
import net.havenkeys.android.ui.items.CategoryScreen
import net.havenkeys.android.ui.items.ItemListViewModel
import net.havenkeys.android.ui.items.ItemsScreen
import net.havenkeys.android.ui.items.TagScreen
import net.havenkeys.android.ui.settings.SettingsNavigation
import net.havenkeys.android.ui.settings.SettingsScreen
import net.havenkeys.android.ui.settings.SettingsViewModel
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.SharedTitle
import net.havenkeys.android.ui.shell.ShellScreens

/**
 * The shell's tab screens. Each `viewModel { }` runs inside its tab's back
 * stack entry, so a ViewModel lives as long as that screen's place in its
 * tab, survives tab switches (saved state), and goes with the shell on lock.
 */
internal fun shellScreens(
    services: NavServices,
    navController: NavHostController,
    open: OpenItem,
    sharedTitle: SharedTitle,
): ShellScreens = ShellScreens(
    home = { padding ->
        HomeScreen(
            viewModel = viewModel {
                HomeViewModel(services.vault, services.accounts, services.events)
            },
            onOpen = open,
            onHealth = { navController.pushOnce(Routes.HEALTH) },
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    items = { padding, onCategory, onTag ->
        ItemsScreen(
            viewModel = itemList(services),
            onCategory = onCategory,
            onTag = onTag,
            contentPadding = padding,
        )
    },
    category = { padding, category, _ ->
        CategoryScreen(
            viewModel = itemList(services),
            category = category,
            onOpen = open,
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    tag = { padding, name, onBack ->
        TagScreen(
            viewModel = itemList(services),
            tag = name,
            onOpen = open,
            onGone = onBack,
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    settings = { padding -> SettingsTab(services, navController, padding) },
)

/** The list behind the Items tab and each category or tag list; each screen has its own. */
@Composable
private fun itemList(services: NavServices): ItemListViewModel =
    viewModel { ItemListViewModel(services.vault, services.accounts, services.events) }

@Composable
private fun SettingsTab(services: NavServices, navController: NavHostController, padding: PaddingValues) {
    val online by services.events.online.collectAsStateWithLifecycle()
    SettingsScreen(
        viewModel = viewModel {
            SettingsViewModel(
                services.settings,
                services.accounts,
                services.vault,
                biometricEnrolled = services.hasBiometricUnlock,
            )
        },
        online = online,
        actions = services.settingsActions(),
        navigation = SettingsNavigation(
            onDevices = { navController.pushOnce(Routes.DEVICES) },
            onAutofillSetup = { navController.pushOnce(Routes.AUTOFILL_SETUP) },
            onPairing = { navController.pushOnce(Routes.PAIRING) },
        ),
        contentPadding = padding,
    )
}
