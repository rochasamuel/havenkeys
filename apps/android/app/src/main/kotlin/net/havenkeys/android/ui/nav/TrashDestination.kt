package net.havenkeys.android.ui.nav

import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavGraphBuilder
import androidx.navigation.compose.composable
import net.havenkeys.android.ui.trash.TrashScreen
import net.havenkeys.android.ui.trash.TrashViewModel

/** The Trash, from Settings. It shows overviews only and opens no item, so it leads nowhere else. */
internal fun NavGraphBuilder.trashScreen(services: NavServices, onBack: () -> Unit, onLock: () -> Unit) {
    composable(Routes.TRASH) {
        val online by services.events.online.collectAsStateWithLifecycle()
        TrashScreen(
            viewModel = viewModel { TrashViewModel(services.vault, services.accounts, services.events) },
            online = online,
            onBack = onBack,
            onLock = onLock,
        )
    }
}
