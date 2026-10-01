package net.havenkeys.android.ui.nav

import androidx.activity.compose.LocalActivity
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.AnimatedVisibilityScope
import androidx.compose.animation.SharedTransitionLayout
import androidx.compose.animation.SharedTransitionScope
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.fragment.app.FragmentActivity
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
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.ui.autofillsetup.AutofillSetupScreen
import net.havenkeys.android.ui.generator.GeneratorScreen
import net.havenkeys.android.ui.generator.GeneratorViewModel
import net.havenkeys.android.ui.item.ItemScreen
import net.havenkeys.android.ui.item.ItemViewModel
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.settings.DevicesScreen
import net.havenkeys.android.ui.settings.DevicesViewModel
import net.havenkeys.android.ui.settings.SettingsNavigation
import net.havenkeys.android.ui.settings.SettingsScreen
import net.havenkeys.android.ui.settings.SettingsViewModel
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel
import net.havenkeys.android.ui.vault.VaultScreen
import net.havenkeys.android.ui.vault.VaultViewModel

@Composable
fun HavenNavHost(container: AppContainer, modifier: Modifier = Modifier) {
    val activity = requireNotNull(LocalActivity.current as? FragmentActivity)
    val root = viewModel { RootViewModel(container.vaultRepository, container.events) }
    val start by root.start.collectAsStateWithLifecycle()
    val first = start
    if (first == null) {
        Surface(modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {}
        return
    }

    val navController = rememberNavController()
    val scope = rememberCoroutineScope()
    val motion = HavenTheme.motion
    SharedTransitionLayout(modifier) {
        val shared = this
        NavHost(
            navController = navController,
            startDestination = routeOf(first),
            enterTransition = { fadeIn(spec(motion)) },
            exitTransition = { fadeOut(spec(motion)) },
            popEnterTransition = { fadeIn(spec(motion)) },
            popExitTransition = { fadeOut(spec(motion)) },
        ) {
            composable(Routes.ONBOARDING) {
                OnboardingScreen(
                    viewModel = viewModel { OnboardingViewModel(container.accountRepository) },
                    onDone = { scope.launch { navController.replaceAll(routeOf(root.current())) } },
                )
            }
            composable(Routes.UNLOCK) {
                UnlockScreen(
                    viewModel = viewModel {
                        UnlockViewModel(
                            container.vaultRepository,
                            biometricAvailable = container.biometricGate.available(activity),
                            hasBundle = container::hasBiometricUnlock,
                            deleteBundle = container::forgetBiometricUnlock,
                        )
                    },
                    activity = activity,
                    container = container,
                    onUnlocked = { navController.replaceAll(Routes.VAULT) },
                )
            }
            vaultScreens(container, navController, shared, motion)
            toolScreens(container, navController, activity)
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
            val route = routeOf(target)
            if (navController.currentDestination?.route != route) navController.replaceAll(route)
        }
    }
}

private fun NavHostController.replaceAll(route: String) =
    navigate(route) { popUpTo(graph.id) { inclusive = true } }

/** Unlock → vault and the lock wipe take the seal's time; reduced motion cuts instantly. */
private fun AnimatedContentTransitionScope<NavBackStackEntry>.spec(motion: HavenMotion): FiniteAnimationSpec<Float> {
    val sealed = initialState.destination.route == Routes.UNLOCK || targetState.destination.route == Routes.UNLOCK
    return if (sealed) motion.sealSpec() else motion.snapSpec()
}

/** The vault list and an item; their titles share an element across the move. */
private fun NavGraphBuilder.vaultScreens(
    container: AppContainer,
    navController: NavHostController,
    shared: SharedTransitionScope,
    motion: HavenMotion,
) {
    composable(Routes.VAULT) {
        val visibility = this
        VaultScreen(
            viewModel = viewModel {
                VaultViewModel(container.vaultRepository, container.accountRepository, container.events)
            },
            onOpen = { id -> navController.navigate(Routes.item(id)) },
            onLock = container.vaultRepository::lock,
            onGenerator = { navController.navigate(Routes.GENERATOR) },
            onSettings = { navController.navigate(Routes.SETTINGS) },
            titleModifier = { id -> Modifier.sharedTitle(shared, id, visibility, motion) },
        )
    }
    composable(Routes.ITEM, arguments = listOf(navArgument(Routes.ITEM_ID) { type = NavType.StringType })) {
        val id = requireNotNull(it.arguments?.getString(Routes.ITEM_ID))
        val online by container.events.online.collectAsStateWithLifecycle()
        ItemScreen(
            viewModel = viewModel {
                ItemViewModel(container.vaultRepository, container.settingsRepository, container.events, id)
            },
            clipboard = container.clipboard,
            online = online,
            onBack = { navController.popBackStack() },
            onLock = container.vaultRepository::lock,
            titleModifier = Modifier.sharedTitle(shared, id, this, motion),
        )
    }
}

/** Generator, Settings and the screens Settings leads to. No route carries an argument. */
private fun NavGraphBuilder.toolScreens(
    container: AppContainer,
    navController: NavHostController,
    activity: FragmentActivity,
) {
    val back: () -> Unit = { navController.popBackStack() }
    val lock: () -> Unit = container.vaultRepository::lock
    composable(Routes.GENERATOR) {
        val online by container.events.online.collectAsStateWithLifecycle()
        GeneratorScreen(
            viewModel = viewModel { GeneratorViewModel(container.vaultRepository, container.settingsRepository) },
            clipboard = container.clipboard,
            online = online,
            onBack = back,
            onLock = lock,
        )
    }
    composable(Routes.SETTINGS) {
        val online by container.events.online.collectAsStateWithLifecycle()
        SettingsScreen(
            viewModel = viewModel {
                SettingsViewModel(
                    container.settingsRepository,
                    container.accountRepository,
                    container.vaultRepository,
                    biometricEnrolled = container::hasBiometricUnlock,
                )
            },
            activity = activity,
            container = container,
            online = online,
            navigation = SettingsNavigation(
                onBack = back,
                onLock = lock,
                onDevices = { navController.navigate(Routes.DEVICES) },
                onAutofillSetup = { navController.navigate(Routes.AUTOFILL_SETUP) },
            ),
        )
    }
    composable(Routes.DEVICES) {
        val online by container.events.online.collectAsStateWithLifecycle()
        DevicesScreen(
            viewModel = viewModel { DevicesViewModel(container.accountRepository, container.events) },
            online = online,
            onBack = back,
            onLock = lock,
        )
    }
    composable(Routes.AUTOFILL_SETUP) {
        val online by container.events.online.collectAsStateWithLifecycle()
        AutofillSetupScreen(online = online, onBack = back, onLock = lock)
    }
}

/** The item's title moves from its list row to its screen; reduced motion cuts instead. */
@Composable
private fun Modifier.sharedTitle(
    shared: SharedTransitionScope,
    id: String,
    visibility: AnimatedVisibilityScope,
    motion: HavenMotion,
): Modifier = if (motion.reduced) {
    this
} else {
    with(shared) {
        this@sharedTitle.sharedBounds(
            rememberSharedContentState(key = "title-$id"),
            visibility,
            resizeMode = SharedTransitionScope.ResizeMode.scaleToBounds(),
        )
    }
}
