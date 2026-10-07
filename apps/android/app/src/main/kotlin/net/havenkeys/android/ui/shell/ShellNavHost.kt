package net.havenkeys.android.ui.shell

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraphBuilder
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
 * ViewModels; tests pass plain text. [items] gets the ways to open a
 * category and a tag, [category] and [tag] the way back.
 */
class ShellScreens(
    val home: @Composable (PaddingValues) -> Unit,
    val items: @Composable (PaddingValues, onCategory: (Category) -> Unit, onTag: (String) -> Unit) -> Unit,
    val category: @Composable (PaddingValues, Category, onBack: () -> Unit) -> Unit,
    val tag: @Composable (PaddingValues, String, onBack: () -> Unit) -> Unit,
    val settings: @Composable (PaddingValues) -> Unit,
)

/**
 * The tag each open tag list shows, by its back stack entry's id (a random
 * UUID). Memory only, never a route, a Bundle or saved state (security
 * model §22.12). It lives with the shell's entry, so the lock, which
 * replaces the shell, drops it; so does a process death, and a tag list
 * that finds no name here backs out to Items.
 */
internal class TagSelections : ViewModel() {
    private val names = mutableMapOf<String, String>()

    operator fun get(entryId: String): String? = names[entryId]

    operator fun set(entryId: String, name: String) {
        names[entryId] = name
    }

    override fun onCleared() = names.clear()
}

/**
 * Opens [name]'s list. Two quick taps on one tag open it once; a tap on
 * another tag goes there at once, as [pushOnce] does for routes with
 * arguments. The back stack changes synchronously, so the new entry is on
 * top when its name is recorded.
 */
internal fun NavHostController.openTag(name: String, selections: TagSelections) {
    val top = currentBackStackEntry
    if (top?.destination?.route == ShellRoutes.TAG && selections[top.id] == name) return
    navigate(ShellRoutes.TAG)
    currentBackStackEntry?.let { selections[it.id] = name }
}

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
    // Scoped to the shell's own entry: it outlives the tabs' back stacks and an item opened over them.
    val selections = viewModel { TagSelections() }
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
        itemsGraph(navController, screens, padding, selections)
        navigation(startDestination = Tab.SETTINGS.root, route = Tab.SETTINGS.graph) {
            composable(Tab.SETTINGS.root) { screens.settings(padding) }
        }
    }
}

/** The Items tab: the categories and tags, and the list each one opens. */
private fun NavGraphBuilder.itemsGraph(
    navController: NavHostController,
    screens: ShellScreens,
    padding: PaddingValues,
    selections: TagSelections,
) {
    navigation(startDestination = Tab.ITEMS.root, route = Tab.ITEMS.graph) {
        composable(Tab.ITEMS.root) {
            // Two quick taps on a category or a tag open its list once.
            screens.items(
                padding,
                { category -> navController.pushOnce(ShellRoutes.category(category)) },
                { name -> navController.openTag(name, selections) },
            )
        }
        composable(ShellRoutes.TAG) { entry ->
            // None after a process death, or for a route opened some other way.
            val name = selections[entry.id]
            if (name.isNullOrBlank()) {
                LaunchedEffect(Unit) { navController.popBackStack() }
            } else {
                screens.tag(padding, name) { navController.popBackStack() }
            }
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
}
