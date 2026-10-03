package net.havenkeys.android.ui.shell

import androidx.annotation.StringRes
import androidx.navigation.NavDestination
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.NavHostController
import net.havenkeys.android.R
import net.havenkeys.android.ui.items.Category
import net.havenkeys.android.ui.kit.HavenIcon
import net.havenkeys.android.ui.nav.Move

/** The bottom bar's tabs (spec §6.4); each is a nested graph with its own back stack. */
enum class Tab(val graph: String, val root: String, @StringRes val label: Int, val icon: HavenIcon) {
    HOME("tab-home", "home", R.string.tab_home, HavenIcon.Home),
    ITEMS("tab-items", "items", R.string.tab_items, HavenIcon.Items),
    SETTINGS("tab-settings", "settings", R.string.settings_title, HavenIcon.Gear),
}

/** Routes inside the shell. A category list carries only its category's name. */
object ShellRoutes {
    const val CATEGORY_ARG = "category"
    const val CATEGORY = "items/{$CATEGORY_ARG}"

    fun category(category: Category) = "items/${category.arg}"
}

/** The tab a destination belongs to. */
internal fun NavDestination.tab(): Tab? =
    hierarchy.firstNotNullOfOrNull { destination -> Tab.entries.firstOrNull { it.graph == destination.route } }

/**
 * A bottom bar tap. Another tab comes back as it was left (its stack saved
 * and restored); the current tab again goes back to its root (spec §6.1).
 */
internal fun NavHostController.selectTab(tab: Tab) {
    if (currentBackStackEntry?.destination?.tab() == tab) {
        popBackStack(tab.root, inclusive = false)
        return
    }
    navigate(tab.graph) {
        popUpTo(graph.findStartDestination().id) { saveState = true }
        launchSingleTop = true
        restoreState = true
    }
}

/** Inside the shell: another tab crossfades, a deeper screen of the same tab is pushed. */
internal fun innerMove(from: NavDestination, to: NavDestination): Move =
    if (from.tab() != to.tab()) Move.TAB else Move.PUSH
