package net.havenkeys.android.ui.nav

import android.net.Uri
import androidx.activity.compose.LocalActivity
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.fragment.app.FragmentActivity
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavGraphBuilder
import androidx.navigation.NavHostController
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R
import net.havenkeys.android.ui.autofillsetup.AutofillSetupScreen
import net.havenkeys.android.ui.edit.EditNavigation
import net.havenkeys.android.ui.edit.EditScreen
import net.havenkeys.android.ui.edit.EditTarget
import net.havenkeys.android.ui.edit.EditViewModel
import net.havenkeys.android.ui.generator.GeneratorScreen
import net.havenkeys.android.ui.generator.GeneratorViewModel
import net.havenkeys.android.ui.health.HealthNavigation
import net.havenkeys.android.ui.health.HealthScreen
import net.havenkeys.android.ui.health.HealthViewModel
import net.havenkeys.android.ui.item.ItemNavigation
import net.havenkeys.android.ui.item.ItemScreen
import net.havenkeys.android.ui.item.ItemViewModel
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.search.SearchScreen
import net.havenkeys.android.ui.search.SearchViewModel
import net.havenkeys.android.ui.pairing.PairingScreen
import net.havenkeys.android.ui.pairing.PairingViewModel
import net.havenkeys.android.ui.settings.DevicesScreen
import net.havenkeys.android.ui.settings.DevicesViewModel
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.ShellNavigation
import net.havenkeys.android.ui.shell.ShellScreen
import net.havenkeys.android.ui.shell.ShellViewModel
import net.havenkeys.android.ui.shell.TrashUndo
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenTheme

/** What every destination of the app's graph needs. */
private class Nav(
    val services: NavServices,
    val controller: NavHostController,
    val shared: SharedTransitionScope,
    val motion: HavenMotion,
    val travel: TitleTravel,
    val trashUndo: TrashUndo,
) {
    /** Opens an item over the shell; its title travels from the tapped row. */
    val open: OpenItem = { id, origin ->
        if (controller.pushOnce(Routes.item(id))) travel.tap(id, origin)
    }

    val back: () -> Unit = { controller.popBackStack() }
    val lock: () -> Unit = services.vault::lock
}

/** The app's navigation over the [AppContainer] and this activity. */
@Composable
fun HavenNavHost(container: AppContainer, modifier: Modifier = Modifier) {
    val activity = requireNotNull(LocalActivity.current as? FragmentActivity)
    HavenNavHost(rememberNavServices(container, activity), modifier)
}

/**
 * The app's navigation: onboarding and unlock outside the shell; the shell
 * and search; and the full-screen screens over the shell (spec §6.1). A lock
 * replaces the whole back stack with Unlock, at once. [navController] is a
 * parameter so a test can watch the back stack.
 */
@Composable
internal fun HavenNavHost(
    services: NavServices,
    modifier: Modifier = Modifier,
    navController: NavHostController = rememberNavController(),
) {
    val root = viewModel { RootViewModel(services.vault, services.events) }
    val start by root.start.collectAsStateWithLifecycle()
    val first = start
    if (first == null) {
        Box(modifier.fillMaxSize().background(HavenTheme.colors.pane))
        return
    }

    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    val travel = remember { TitleTravel() }
    // One per graph: the item screen hands its Delete to the screen it pops back to.
    val trashUndo = remember(services) {
        TrashUndo(services.vault::restore, sync = { services.accounts.syncNow(fresh = true) })
    }
    // The window's ground shows through a screen that fades under a push, which reads as dimmed.
    SharedTransitionLayout(modifier.background(HavenTheme.colors.pane)) {
        val nav = Nav(services, navController, this, motion, travel, trashUndo)
        NavHost(
            navController = navController,
            startDestination = routeOf(first),
            enterTransition = {
                enterFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = false)
            },
            exitTransition = {
                exitFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = false)
            },
            popEnterTransition = {
                enterFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = true)
            },
            popExitTransition = {
                exitFor(outerMove(initialState.destination.route, targetState.destination.route), motion, pop = true)
            },
        ) {
            entryScreens(nav, root, scope)
            shellAndSearch(nav)
            itemScreens(nav)
            toolScreens(nav)
        }
    }

    // A back stack restored from before a process kill must not outlive the
    // lock. Decided from Rust's state now, not `first`: after a rotation the
    // vault may have been unlocked or created since the start was read.
    LaunchedEffect(navController) {
        routeToForce(navController.currentDestination?.route, root.current())?.let(navController::replaceAll)
    }
    // The lock wipe: no screen that showed vault data stays in the back stack.
    LaunchedEffect(navController) {
        root.lockedSignal.collect { target ->
            travel.clear()
            val route = routeOf(target)
            if (navController.currentDestination?.route != route) navController.replaceAll(route)
        }
    }
}

/**
 * Pushes [route] unless the screen on top is already that route with the
 * same arguments. Two quick taps on one target open it once; a tap on
 * another target goes there at once, even mid-transition (spec §7). The
 * back stack changes synchronously, so the second tap of a pair sees the
 * first one's entry on top. Returns whether it pushed.
 */
internal fun NavHostController.pushOnce(route: String): Boolean {
    if (currentBackStackEntry?.concreteRoute() == route) return false
    navigate(route)
    return true
}

/**
 * This entry's route with its arguments filled in ("item/{id}" → "item/abc"),
 * encoded as a route is built, so it equals the route that opened it. Ids,
 * kinds and categories encode to themselves.
 */
internal fun NavBackStackEntry.concreteRoute(): String? {
    val pattern = destination.route ?: return null
    return ArgumentSlot.replace(pattern) { slot ->
        arguments?.getString(slot.groupValues[1])?.let(Uri::encode) ?: slot.value
    }
}

private val ArgumentSlot = Regex("\\{([^}]+)\\}")

private fun NavHostController.replaceAll(route: String) =
    navigate(route) { popUpTo(graph.id) { inclusive = true } }

/** Onboarding and unlock, outside the shell. */
private fun NavGraphBuilder.entryScreens(nav: Nav, root: RootViewModel, scope: CoroutineScope) {
    composable(Routes.ONBOARDING) {
        val accountDeleted by nav.services.events.accountDeleted.collectAsStateWithLifecycle()
        OnboardingScreen(
            viewModel = viewModel { OnboardingViewModel(nav.services.accounts) },
            onDone = { scope.launch { nav.controller.replaceAll(routeOf(root.current())) } },
            accountDeleted = accountDeleted,
            onAccountDeletedSeen = nav.services.events::accountDeletedSeen,
        )
    }
    composable(Routes.UNLOCK) {
        nav.services.unlockScreen { nav.controller.replaceAll(Routes.SHELL) }
    }
}

/** The shell and search; the pill and the field are one shared element. */
private fun NavGraphBuilder.shellAndSearch(nav: Nav) {
    val services = nav.services
    composable(Routes.SHELL) {
        ShellScreen(
            viewModel = viewModel {
                ShellViewModel(services.vault, services.accounts, services.events)
            },
            screens = shellScreens(
                nav.services,
                nav.controller,
                nav.open,
                nav.travel.from(nav.shared, this, nav.motion),
            ),
            navigation = ShellNavigation(
                onSearch = { nav.controller.pushOnce(Routes.SEARCH) },
                onNew = { kind -> nav.controller.pushOnce(Routes.new(kind)) },
                onGenerator = { nav.controller.pushOnce(Routes.GENERATOR) },
            ),
            searchPillModifier = Modifier.sharedIfMoving(nav.shared, SEARCH_KEY, this, nav.motion),
            trashUndo = nav.trashUndo,
        )
    }
    composable(Routes.SEARCH) {
        SearchScreen(
            viewModel = viewModel { SearchViewModel(services.vault, services.events) },
            onOpen = nav.open,
            onCancel = nav.back,
            fieldModifier = Modifier.sharedIfMoving(
                nav.shared,
                SEARCH_KEY,
                this,
                nav.motion,
                SharedTransitionScope.ResizeMode.RemeasureToBounds,
            ),
            sharedTitle = nav.travel.from(nav.shared, this, nav.motion),
            trashUndo = nav.trashUndo,
        )
    }
}

/** An item and its editors, over the shell. */
private fun NavGraphBuilder.itemScreens(nav: Nav) {
    val services = nav.services
    composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        val online by services.events.online.collectAsStateWithLifecycle()
        ItemScreen(
            viewModel = viewModel {
                ItemViewModel(services.vault, services.settings, services.events, id)
            },
            clipboard = services.clipboard,
            online = online,
            navigation = ItemNavigation(
                onBack = nav.back,
                onLock = nav.lock,
                onEdit = { nav.controller.pushOnce(Routes.edit(id)) },
                // The screen it pops back to (the shell, search or vault health) says so, with Undo.
                onTrashed = { title ->
                    nav.trashUndo.offer(id, title)
                    nav.back()
                },
                onDeleted = { title ->
                    nav.trashUndo.deletedForGood(id, title)
                    nav.back()
                },
            ),
            titleModifier = Modifier.sharedIfMoving(nav.shared, titleKey(id), this, nav.motion),
            tileModifier = Modifier.sharedIfMoving(nav.shared, tileKey(id), this, nav.motion),
        )
    }
    composable(Routes.EDIT, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        EditRoute(nav, EditTarget.Existing(id))
    }
    composable(Routes.NEW, arguments = listOf(navArgument(Routes.KIND) { type = NavType.StringType })) {
        val kind = it.arguments?.getString(Routes.KIND)?.let(::creatableKind)
        if (kind == null) {
            LaunchedEffect(Unit) { nav.controller.popBackStack() }
        } else {
            EditRoute(nav, EditTarget.New(kind))
        }
    }
}

@Composable
private fun EditRoute(nav: Nav, target: EditTarget) {
    val services = nav.services
    val online by services.events.online.collectAsStateWithLifecycle()
    EditScreen(
        viewModel = viewModel {
            EditViewModel(services.vault, services.accounts, services.events, target)
        },
        isNew = target is EditTarget.New,
        online = online,
        navigation = EditNavigation(
            onDone = { id ->
                if (target is EditTarget.New) {
                    // The new item's screen replaces the editor, so Back goes to where the add began.
                    nav.controller.navigate(Routes.item(id)) { popUpTo(Routes.NEW) { inclusive = true } }
                } else {
                    nav.controller.popBackStack()
                }
            },
            onBack = nav.back,
            onLock = nav.lock,
        ),
    )
}

/**
 * The generator, vault health, and the screens Settings leads to; the Trash
 * shows overviews only and opens no item. No route carries an argument.
 */
private fun NavGraphBuilder.toolScreens(nav: Nav) {
    val services = nav.services
    composable(Routes.GENERATOR) {
        val online by services.events.online.collectAsStateWithLifecycle()
        GeneratorScreen(
            viewModel = viewModel { GeneratorViewModel(services.vault, services.settings) },
            clipboard = services.clipboard,
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.DEVICES) {
        val online by services.events.online.collectAsStateWithLifecycle()
        DevicesScreen(
            viewModel = viewModel { DevicesViewModel(services.accounts, services.events) },
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.PAIRING) {
        val online by services.events.online.collectAsStateWithLifecycle()
        val title = stringResource(R.string.pairing_verify_title)
        PairingScreen(
            viewModel = viewModel { PairingViewModel(services.accounts) },
            online = online,
            canVerify = services.canVerifyUser,
            verifyUser = { _, subtitle -> services.verifyUser(title, subtitle) },
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.HEALTH) {
        val online by services.events.online.collectAsStateWithLifecycle()
        HealthScreen(
            viewModel = viewModel { HealthViewModel(services.vault, services.events) },
            online = online,
            navigation = HealthNavigation(
                onOpen = { id -> nav.controller.pushOnce(Routes.item(id)) },
                onEdit = { id -> nav.controller.pushOnce(Routes.edit(id)) },
                onBack = nav.back,
                onLock = nav.lock,
            ),
            trashUndo = nav.trashUndo,
        )
    }
    composable(Routes.AUTOFILL_SETUP) {
        val online by services.events.online.collectAsStateWithLifecycle()
        AutofillSetupScreen(online = online, onBack = nav.back, onLock = nav.lock)
    }
    trashScreen(services, onBack = nav.back, onLock = nav.lock)
}
