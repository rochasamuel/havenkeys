package net.havenkeys.android.credentials

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.annotation.RequiresApi
import androidx.credentials.provider.BeginGetCredentialRequest
import androidx.credentials.provider.BeginGetCredentialResponse
import androidx.credentials.provider.PendingIntentHandler
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.autofill.cancel
import net.havenkeys.android.autofill.unlockThen
import net.havenkeys.android.security.hardenWindow

/**
 * "Unlock HavenKeys" in Android's sheet (spec §5.3). After unlocking, the
 * entries for the same request go back to Android, which shows them.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class CredentialUnlockActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        hardenWindow()
        val request = PendingIntentHandler.retrieveBeginGetCredentialRequest(intent) ?: return cancel()
        unlockThen(container) {
            lifecycleScope.launch { answer(request) }
        }
    }

    /** An unexpected failure answers with no entries instead of crashing the unlocked app. */
    @Suppress("TooGenericExceptionCaught", "SwallowedException")
    private suspend fun answer(request: BeginGetCredentialRequest) {
        val response = try {
            answerBeginGet(this, request, true, container.credentialRepository)
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // The message is never kept: it could quote anything.
            BeginGetCredentialResponse()
        }
        val result = Intent()
        PendingIntentHandler.setBeginGetCredentialResponse(result, response)
        setResult(Activity.RESULT_OK, result)
        finish()
    }
}
