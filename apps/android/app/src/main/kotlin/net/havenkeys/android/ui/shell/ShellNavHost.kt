package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.navigation
import androidx.navigation.navArgument
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.nav.enterFor
import net.havenkeys.android.ui.nav.exitFor
import net.havenkeys.android.ui.nav.pushOnce
import net.havenkeys.android.ui.theme.HavenTheme

/** How far a tab's content rises as it fades in (spec §7: a few dp). */
private val TabShift = 6.dp

/**
 * The shell's tab screens. The app's NavHost builds them with their
 * ViewModels; tests pass plain text. [items] gets the way to open a
 * category, [category] the way back.
 */
class ShellScreens(
    val home: @Composable (PaddingValues) -> Unit,
    val items: @Composable (PaddingValues, onCategory: (Category) -> Unit) -> Unit,
    val category: @Composable (PaddingValues, Category, onBack: () -> Unit) -> Unit,
    val settings: @Composable (PaddingValues) -> Unit,
)

/**
 * The shell's content: one nested graph per tab, so each tab keeps its own
 * back stack (spec §6.1). [padding] is the room the scaffold leaves for the
 * floating button.
 */
@Composable
fun ShellNavHost(
    navController: NavHostController,
    screens: ShellScreens,
    padding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val motion = HavenTheme.motion
    val shift = with(LocalDensity.current) { TabShift.roundToPx() }
    NavHost(
        navController = navController,
        startDestination = Tab.HOME.graph,
        modifier = modifier,
        enterTransition = {
            val move = innerMove(initialState.destination, targetState.destination)
            enterFor(move, motion, pop = false, tabShiftPx = shift)
        },
        exitTransition = {
            exitFor(innerMove(initialState.destination, targetState.destination), motion, pop = false)
        },
        popEnterTransition = {
            val move = innerMove(initialState.destination, targetState.destination)
            enterFor(move, motion, pop = true, tabShiftPx = shift)
        },
        popExitTransition = {
            exitFor(innerMove(initialState.destination, targetState.destination), motion, pop = true)
        },
    ) {
        navigation(startDestination = Tab.HOME.root, route = Tab.HOME.graph) {
            composable(Tab.HOME.root) { screens.home(padding) }
        }
        navigation(startDestination = Tab.ITEMS.root, route = Tab.ITEMS.graph) {
            composable(Tab.ITEMS.root) {
                // Two quick taps on a category open its list once.
                screens.items(padding) { category -> navController.pushOnce(ShellRoutes.category(category)) }
            }
            composable(
                ShellRoutes.CATEGORY,
                arguments = listOf(navArgument(ShellRoutes.CATEGORY_ARG) { type = NavType.StringType }),
            ) { entry ->
                val category = Category.fromArg(entry.arguments?.getString(ShellRoutes.CATEGORY_ARG))
                if (category == null) {
                    LaunchedEffect(Unit) { navController.popBackStack() }
                } else {
                    screens.category(padding, category) { navController.popBackStack() }
                }
            }
        }
        navigation(startDestination = Tab.SETTINGS.root, route = Tab.SETTINGS.graph) {
            composable(Tab.SETTINGS.root) { screens.settings(padding) }
        }
    }
}
