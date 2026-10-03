package net.havenkeys.android.ui.nav

import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavHostController
import net.havenkeys.android.ui.home.HomeScreen
import net.havenkeys.android.ui.home.HomeViewModel
import net.havenkeys.android.ui.items.CategoryScreen
import net.havenkeys.android.ui.items.ItemListViewModel
import net.havenkeys.android.ui.items.ItemsScreen
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
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    items = { padding, onCategory ->
        ItemsScreen(
            viewModel = viewModel {
                ItemListViewModel(services.vault, services.accounts, services.events)
            },
            onCategory = onCategory,
            contentPadding = padding,
        )
    },
    category = { padding, category, onBack ->
        CategoryScreen(
            viewModel = viewModel {
                ItemListViewModel(services.vault, services.accounts, services.events)
            },
            category = category,
            onOpen = open,
            onBack = onBack,
            contentPadding = padding,
            sharedTitle = sharedTitle,
        )
    },
    settings = { padding ->
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
            ),
            contentPadding = padding,
        )
    },
)
