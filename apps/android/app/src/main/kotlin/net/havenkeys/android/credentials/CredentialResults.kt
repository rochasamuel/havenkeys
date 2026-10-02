package net.havenkeys.android.credentials

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.widget.Toast
import androidx.annotation.RequiresApi
import androidx.credentials.CreateCredentialResponse
import androidx.credentials.GetCredentialResponse
import androidx.credentials.exceptions.CreateCredentialException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.provider.PendingIntentHandler
import androidx.fragment.app.FragmentActivity
import net.havenkeys.android.AppContainer
import net.havenkeys.android.R

/** The answer goes back to Android as the provider API requires; nothing else carries it. */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal fun Activity.finishGet(response: GetCredentialResponse) {
    val result = Intent()
    PendingIntentHandler.setGetCredentialResponse(result, response)
    setResult(Activity.RESULT_OK, result)
    finish()
}

@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal fun Activity.failGet(e: GetCredentialException) {
    val result = Intent()
    PendingIntentHandler.setGetCredentialException(result, e)
    setResult(Activity.RESULT_OK, result)
    finish()
}

@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal fun Activity.finishCreate(response: CreateCredentialResponse) {
    val result = Intent()
    PendingIntentHandler.setCreateCredentialResponse(result, response)
    setResult(Activity.RESULT_OK, result)
    finish()
}

@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
internal fun Activity.failCreate(e: CreateCredentialException) {
    val result = Intent()
    PendingIntentHandler.setCreateCredentialException(result, e)
    setResult(Activity.RESULT_OK, result)
    finish()
}

/**
 * Passkey user verification (spec §8.2): a real prompt on every use, with
 * the screen lock allowed. An unlock on this same screen already verified
 * the user, so it is not asked twice. No screen lock at all: refused, since
 * HavenKeys asserts UV in every signature.
 */
internal suspend fun FragmentActivity.userVerified(
    container: AppContainer,
    unlockedHere: Boolean,
    subtitle: String,
): Boolean = when {
    unlockedHere -> true
    !container.biometricGate.canVerifyUser(this) -> {
        Toast.makeText(applicationContext, R.string.passkey_needs_screen_lock, Toast.LENGTH_LONG).show()
        false
    }
    else -> container.biometricGate.verifyUser(this, getString(R.string.passkey_verify_title), subtitle)
}
