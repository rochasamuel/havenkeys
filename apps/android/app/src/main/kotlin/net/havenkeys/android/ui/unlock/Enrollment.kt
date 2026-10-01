package net.havenkeys.android.ui.unlock

import androidx.fragment.app.FragmentActivity
import java.io.IOException
import java.security.GeneralSecurityException
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.security.BootCount

/**
 * Turns on biometric unlock (spec §5.2): Rust makes the bundle from the
 * master password, a biometric prompt unlocks a fresh Keystore key, and the
 * sealed bundle is stored. Call from the main thread (BiometricPrompt).
 * Any earlier bundle is replaced; on failure there is none.
 */
suspend fun enrollBiometrics(activity: FragmentActivity, container: AppContainer, password: String): Outcome<Unit> {
    val bundle = when (val r = container.vaultRepository.createUnlockBundle(password, BootCount.current(activity))) {
        is Outcome.Ok -> r.value
        is Outcome.Failed -> return Outcome.Failed(enrollmentErrorCode(r.code))
    }
    var result: Outcome<Unit> = Outcome.Failed(KEYSTORE)
    try {
        val cipher = container.biometricKeys.encryptCipher()
        val unlocked = container.biometricGate.authenticate(
            activity,
            activity.getString(R.string.biometric_enroll_title),
            activity.getString(R.string.biometric_enroll_cancel),
            cipher,
        )
        if (unlocked == null) {
            result = Outcome.Failed(CANCELLED)
        } else {
            container.biometricKeys.store(unlocked, bundle)
            result = Outcome.Ok(Unit)
        }
    } catch (@Suppress("SwallowedException") e: IllegalStateException) {
        // The Keystore refuses a biometric-bound key while no fingerprint or face is enrolled.
        result = Outcome.Failed(BIOMETRIC_UNAVAILABLE)
    } catch (@Suppress("SwallowedException") e: GeneralSecurityException) {
        result = Outcome.Failed(KEYSTORE)
    } catch (@Suppress("SwallowedException") e: IOException) {
        result = Outcome.Failed(KEYSTORE)
    } finally {
        bundle.fill(0)
        if (result !is Outcome.Ok) container.forgetBiometricUnlock()
    }
    return result
}

/**
 * Rust refuses to make a bundle when the boot count is unknown: that is
 * this phone, not a wrong password (`unlock_failed` stays as it is).
 */
internal fun enrollmentErrorCode(code: String): String =
    if (code == "bundle_refused") BIOMETRIC_UNAVAILABLE else code

private const val BIOMETRIC_UNAVAILABLE = "biometric_unavailable"
private const val KEYSTORE = "keychain_unavailable"
private const val CANCELLED = "cancelled"
