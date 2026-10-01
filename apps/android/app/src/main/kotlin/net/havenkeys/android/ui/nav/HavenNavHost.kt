package net.havenkeys.android.ui.nav

import androidx.activity.compose.LocalActivity
import androidx.compose.animation.AnimatedContentTransitionScope
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import kotlinx.coroutines.launch
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R
import net.havenkeys.android.ui.components.HavenTopBar
import net.havenkeys.android.ui.onboarding.OnboardingScreen
import net.havenkeys.android.ui.onboarding.OnboardingViewModel
import net.havenkeys.android.ui.theme.HavenMotion
import net.havenkeys.android.ui.theme.HavenTheme
import net.havenkeys.android.ui.unlock.UnlockScreen
import net.havenkeys.android.ui.unlock.UnlockViewModel

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
    NavHost(
        navController = navController,
        startDestination = routeOf(first),
        modifier = modifier,
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
        composable(Routes.VAULT) { VaultPlaceholder(container) }
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

private fun routeOf(start: Start) = when (start) {
    Start.ONBOARDING -> Routes.ONBOARDING
    Start.UNLOCK -> Routes.UNLOCK
    Start.VAULT -> Routes.VAULT
}

/** Unlock → vault and the lock wipe take the seal's time; reduced motion cuts instantly. */
private fun AnimatedContentTransitionScope<NavBackStackEntry>.spec(motion: HavenMotion): FiniteAnimationSpec<Float> {
    val sealed = initialState.destination.route == Routes.UNLOCK || targetState.destination.route == Routes.UNLOCK
    return if (sealed) motion.sealSpec() else motion.snapSpec()
}

/** Stands in for the vault list until it exists (Task 20); only offers Lock. */
@Composable
private fun VaultPlaceholder(container: AppContainer) {
    val online by container.events.online.collectAsStateWithLifecycle()
    Scaffold(
        topBar = {
            HavenTopBar(
                title = stringResource(R.string.app_name),
                online = online,
                onLock = container.vaultRepository::lock,
            )
        },
    ) { padding -> Surface(Modifier.padding(padding).fillMaxSize()) {} }
}
