package net.havenkeys.android.ui.settings

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.AppContainer
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.ui.unlock.enrollBiometrics

/** What Settings needs from outside its ViewModel: biometric enrolment runs a prompt on the activity. */
class SettingsActions(
    val biometricAvailable: Boolean,
    val forgetBiometric: () -> Unit,
    /** Called on the main thread (BiometricPrompt requires it); the password goes straight to Rust. */
    val enrollBiometric: suspend (password: String) -> Outcome<Unit>,
    /**
     * Biometrics or the screen lock before deleting the account; true when the
     * phone has neither (the master password is still required). Main thread.
     */
    val verifyUser: suspend (title: String) -> Boolean,
)

/** The real actions: the container's biometric gate and keys, a prompt on [activity]. */
@Composable
fun rememberSettingsActions(container: AppContainer, activity: FragmentActivity): SettingsActions =
    remember(container, activity) {
        SettingsActions(
            biometricAvailable = container.biometricGate.available(activity),
            forgetBiometric = container::forgetBiometricUnlock,
            enrollBiometric = { password -> enrollBiometrics(activity, container, password) },
            verifyUser = { title ->
                !container.biometricGate.canVerifyUser(activity) ||
                    container.biometricGate.verifyUser(activity, title, "")
            },
        )
    }
