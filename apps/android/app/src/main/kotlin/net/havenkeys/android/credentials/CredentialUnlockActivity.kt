package net.havenkeys.android.credentials

import android.app.Activity
import android.content.Intent
import android.os.Build
import android.os.Bundle
import androidx.annotation.RequiresApi
import androidx.credentials.provider.PendingIntentHandler
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.lifecycleScope
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
            lifecycleScope.launch {
                val repo = container.credentialRepository
                val response = answerBeginGet(this@CredentialUnlockActivity, request, true, repo)
                val result = Intent()
                PendingIntentHandler.setBeginGetCredentialResponse(result, response)
                setResult(Activity.RESULT_OK, result)
                finish()
            }
        }
    }
}
