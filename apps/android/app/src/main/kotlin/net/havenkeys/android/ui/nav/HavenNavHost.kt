package net.havenkeys.android.ui.nav

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
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
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
import net.havenkeys.android.ui.autofillsetup.AutofillSetupScreen
import net.havenkeys.android.ui.edit.EditNavigation
import net.havenkeys.android.ui.edit.EditScreen
import net.havenkeys.android.ui.edit.EditTarget
import net.havenkeys.android.ui.edit.EditViewModel
import net.havenkeys.android.ui.generator.GeneratorScreen
import net.havenkeys.android.ui.generator.GeneratorViewModel
import net.havenkeys.android.ui.item.ItemNavigation
import net.havenkeys.android.ui.item.ItemScreen
import net.havenkeys.android.ui.item.ItemViewModel
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.search.SearchScreen
import net.havenkeys.android.ui.search.SearchViewModel
import net.havenkeys.android.ui.settings.DevicesScreen
import net.havenkeys.android.ui.settings.DevicesViewModel
import net.havenkeys.android.ui.shell.OpenItem
import net.havenkeys.android.ui.shell.ShellNavigation
import net.havenkeys.android.ui.shell.ShellScreen
import net.havenkeys.android.ui.shell.ShellViewModel
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel

/** What every destination of the app's graph needs. */
private class Nav(
    val container: AppContainer,
    val controller: NavHostController,
    val activity: FragmentActivity,
    val shared: SharedTransitionScope,
    val motion: HavenMotion,
    val travel: TitleTravel,
) {
    /** Opens an item over the shell; its title travels from the tapped row. */
    val open: OpenItem = { id, origin ->
        travel.tap(id, origin)
        controller.navigate(Routes.item(id))
    }

    val back: () -> Unit = { controller.popBackStack() }
    val lock: () -> Unit = container.vaultRepository::lock
}

/**
 * The app's navigation: onboarding and unlock outside the shell; the shell
 * and search; and the full-screen screens over the shell (spec §6.1). A lock
 * replaces the whole back stack with Unlock, at once.
 */
@Composable
fun HavenNavHost(container: AppContainer, modifier: Modifier = Modifier) {
    val activity = requireNotNull(LocalActivity.current as? FragmentActivity)
    val root = viewModel { RootViewModel(container.vaultRepository, container.events) }
    val start by root.start.collectAsStateWithLifecycle()
    val first = start
    if (first == null) {
        Box(modifier.fillMaxSize().background(HavenTheme.colors.pane))
        return
    }

    val navController = rememberNavController()
    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    val travel = remember { TitleTravel() }
    // The window's ground shows through a screen that fades under a push, which reads as dimmed.
    SharedTransitionLayout(modifier.background(HavenTheme.colors.pane)) {
        val nav = Nav(container, navController, activity, this, motion, travel)
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

private fun NavHostController.replaceAll(route: String) =
    navigate(route) { popUpTo(graph.id) { inclusive = true } }

/** Onboarding and unlock, outside the shell. */
private fun NavGraphBuilder.entryScreens(nav: Nav, root: RootViewModel, scope: CoroutineScope) {
    val container = nav.container
    composable(Routes.ONBOARDING) {
        OnboardingScreen(
            viewModel = viewModel { OnboardingViewModel(container.accountRepository) },
            onDone = { scope.launch { nav.controller.replaceAll(routeOf(root.current())) } },
        )
    }
    composable(Routes.UNLOCK) {
        UnlockScreen(
            viewModel = viewModel {
                UnlockViewModel(
                    container.vaultRepository,
                    biometricAvailable = container.biometricGate.available(nav.activity),
                    hasBundle = container::hasBiometricUnlock,
                    deleteBundle = container::forgetBiometricUnlock,
                )
            },
            activity = nav.activity,
            container = container,
            onUnlocked = { nav.controller.replaceAll(Routes.SHELL) },
        )
    }
}

/** The shell and search; the pill and the field are one shared element. */
private fun NavGraphBuilder.shellAndSearch(nav: Nav) {
    val container = nav.container
    composable(Routes.SHELL) {
        ShellScreen(
            viewModel = viewModel {
                ShellViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            screens = shellScreens(
                container,
                nav.activity,
                nav.controller,
                nav.open,
                nav.travel.from(nav.shared, this, nav.motion),
            ),
            navigation = ShellNavigation(
                onSearch = { nav.controller.navigate(Routes.SEARCH) },
                onNew = { kind -> nav.controller.navigate(Routes.new(kind)) },
                onGenerator = { nav.controller.navigate(Routes.GENERATOR) },
            ),
            searchPillModifier = Modifier.sharedIfMoving(nav.shared, SEARCH_KEY, this, nav.motion),
        )
    }
    composable(Routes.SEARCH) {
        SearchScreen(
            viewModel = viewModel { SearchViewModel(container.vaultRepository, container.events) },
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
        )
    }
}

/** An item and its editors, over the shell. */
private fun NavGraphBuilder.itemScreens(nav: Nav) {
    val container = nav.container
    composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        val online by container.events.online.collectAsStateWithLifecycle()
        ItemScreen(
            viewModel = viewModel {
                ItemViewModel(container.vaultRepository, container.settingsRepository, container.events, id)
            },
            clipboard = container.clipboard,
            online = online,
            navigation = ItemNavigation(
                onBack = nav.back,
                onLock = nav.lock,
                onEdit = { nav.controller.navigate(Routes.edit(id)) },
                onDeleted = nav.back,
            ),
            titleModifier = Modifier.sharedIfMoving(nav.shared, titleKey(id), this, nav.motion),
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
    val container = nav.container
    val online by container.events.online.collectAsStateWithLifecycle()
    EditScreen(
        viewModel = viewModel {
            EditViewModel(container.vaultRepository, container.accountRepository, container.events, target)
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

/** The generator, and the screens Settings leads to. No route carries an argument. */
private fun NavGraphBuilder.toolScreens(nav: Nav) {
    val container = nav.container
    composable(Routes.GENERATOR) {
        val online by container.events.online.collectAsStateWithLifecycle()
        GeneratorScreen(
            viewModel = viewModel { GeneratorViewModel(container.vaultRepository, container.settingsRepository) },
            clipboard = container.clipboard,
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.DEVICES) {
        val online by container.events.online.collectAsStateWithLifecycle()
        DevicesScreen(
            viewModel = viewModel { DevicesViewModel(container.accountRepository, container.events) },
            online = online,
            onBack = nav.back,
            onLock = nav.lock,
        )
    }
    composable(Routes.AUTOFILL_SETUP) {
        val online by container.events.online.collectAsStateWithLifecycle()
        AutofillSetupScreen(online = online, onBack = nav.back, onLock = nav.lock)
    }
}
