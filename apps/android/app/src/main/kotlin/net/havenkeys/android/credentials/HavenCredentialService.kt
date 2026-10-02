package net.havenkeys.android.credentials

import android.content.Intent
import android.os.Build
import android.os.CancellationSignal
import android.os.OutcomeReceiver
import androidx.annotation.RequiresApi
import androidx.credentials.exceptions.ClearCredentialException
import androidx.credentials.exceptions.CreateCredentialException
import androidx.credentials.exceptions.CreateCredentialUnknownException
import androidx.credentials.exceptions.GetCredentialException
import androidx.credentials.exceptions.GetCredentialUnknownException
import androidx.credentials.provider.BeginCreateCredentialRequest
import androidx.credentials.provider.BeginCreateCredentialResponse
import androidx.credentials.provider.BeginCreatePublicKeyCredentialRequest
import androidx.credentials.provider.BeginGetCredentialRequest
import androidx.credentials.provider.BeginGetCredentialResponse
import androidx.credentials.provider.CreateEntry
import androidx.credentials.provider.CredentialProviderService
import androidx.credentials.provider.ProviderClearCredentialStateRequest
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R

/**
 * Android's entry point for Credential Manager (spec §8). Lists what Rust
 * offers for the caller; creating and using happen in our activities after
 * the user's tap. Not app use: nothing here touches the idle timer.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class HavenCredentialService : CredentialProviderService() {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val container get() = (application as HavenApp).container

    override fun onBeginGetCredentialRequest(
        request: BeginGetCredentialRequest,
        cancellationSignal: CancellationSignal,
        callback: OutcomeReceiver<BeginGetCredentialResponse, GetCredentialException>,
    ) {
        val work = scope.launch {
            val answer = try {
                answerBeginGet(
                    this@HavenCredentialService,
                    request,
                    container.events.unlocked.value,
                    container.credentialRepository,
                )
            } catch (e: CancellationException) {
                throw e
            } catch (@Suppress("TooGenericExceptionCaught", "SwallowedException") e: Exception) {
                // The message is never kept: it could quote anything.
                callback.onError(GetCredentialUnknownException())
                return@launch
            }
            callback.onResult(answer)
        }
        cancellationSignal.setOnCancelListener { work.cancel() }
    }

    /** One entry: the activity it opens unlocks if needed and asks Rust. */
    override fun onBeginCreateCredentialRequest(
        request: BeginCreateCredentialRequest,
        cancellationSignal: CancellationSignal,
        callback: OutcomeReceiver<BeginCreateCredentialResponse, CreateCredentialException>,
    ) {
        val entries = try {
            createEntries(request)
        } catch (@Suppress("TooGenericExceptionCaught", "SwallowedException") e: Exception) {
            // The message is never kept: it could quote anything.
            return callback.onError(CreateCredentialUnknownException())
        }
        callback.onResult(BeginCreateCredentialResponse(createEntries = entries))
    }

    private fun createEntries(request: BeginCreateCredentialRequest): List<CreateEntry> =
        if (request is BeginCreatePublicKeyCredentialRequest) {
            listOf(
                CreateEntry(
                    accountName = getString(R.string.app_name),
                    pendingIntent = pendingIntent(this, Intent(this, PasskeyCreateActivity::class.java)),
                    description = getString(R.string.credential_create_entry),
                ),
            )
        } else {
            emptyList()
        }

    override fun onClearCredentialStateRequest(
        request: ProviderClearCredentialStateRequest,
        cancellationSignal: CancellationSignal,
        callback: OutcomeReceiver<Void?, ClearCredentialException>,
    ) {
        // HavenKeys keeps no per-app sign-in state to clear.
        callback.onResult(null)
    }

    override fun onDestroy() {
        scope.cancel()
        super.onDestroy()
    }
}
