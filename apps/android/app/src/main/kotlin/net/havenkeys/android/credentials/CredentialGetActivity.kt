package net.havenkeys.android.credentials

import android.os.Build
import android.os.Bundle
import androidx.annotation.RequiresApi
import androidx.credentials.GetCredentialResponse
import androidx.credentials.GetPasswordOption
import androidx.credentials.GetPublicKeyCredentialOption
import androidx.credentials.PasswordCredential
import androidx.credentials.PublicKeyCredential
import androidx.credentials.exceptions.GetCredentialCancellationException
import androidx.credentials.exceptions.GetCredentialUnknownException
import androidx.credentials.provider.PendingIntentHandler
import androidx.credentials.provider.ProviderGetCredentialRequest
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.autofill.unlockThen
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.security.hardenWindow
import uniffi.havenkeys_mobile.CredentialCaller

/**
 * A tapped passkey or password in Android's sheet. The caller is the one
 * Android attaches now; the extras only name the entry, and Rust re-checks
 * it for this caller. Not exported: only our PendingIntents reach it.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class CredentialGetActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container
    private var answering = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        hardenWindow()
        val request = PendingIntentHandler.retrieveProviderGetCredentialRequest(intent)
        val itemId = intent.getStringExtra(CredentialExtras.ITEM_ID)
        if (request == null || itemId == null) return failGet(GetCredentialUnknownException())
        val caller = request.callingAppInfo.toCaller()
        unlockThen(container) { unlockedHere ->
            if (answering) return@unlockThen
            answering = true
            lifecycleScope.launch { answer(request, caller, itemId, unlockedHere) }
        }
    }

    /** An unexpected failure must still answer Android, and must not take the unlocked app down. */
    @Suppress("TooGenericExceptionCaught", "SwallowedException")
    private suspend fun answer(
        request: ProviderGetCredentialRequest,
        caller: CredentialCaller,
        itemId: String,
        unlockedHere: Boolean,
    ) {
        try {
            when (intent.getStringExtra(CredentialExtras.KIND)) {
                CredentialExtras.KIND_PASSKEY -> signIn(request, caller, itemId, unlockedHere)
                CredentialExtras.KIND_PASSWORD -> password(request, caller, itemId)
                else -> failGet(GetCredentialUnknownException())
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: Exception) {
            // The message is never kept: it could quote anything.
            failGet(GetCredentialUnknownException())
        }
    }

    private suspend fun signIn(
        request: ProviderGetCredentialRequest,
        caller: CredentialCaller,
        itemId: String,
        unlockedHere: Boolean,
    ) {
        val option = request.credentialOptions.filterIsInstance<GetPublicKeyCredentialOption>().firstOrNull()
        val credentialId = intent.getByteArrayExtra(CredentialExtras.CREDENTIAL_ID)
        if (option == null || credentialId == null) return failGet(GetCredentialUnknownException())
        val title = intent.getStringExtra(CredentialExtras.TITLE).orEmpty()
        if (!userVerified(container, unlockedHere, getString(R.string.passkey_verify_sign_in, title))) {
            return failGet(GetCredentialCancellationException())
        }
        val signed = container.credentialRepository.passkeySignIn(
            caller,
            option.requestJson,
            option.clientDataHash,
            itemId,
            credentialId,
        )
        when (signed) {
            is Outcome.Ok -> finishGet(GetCredentialResponse(PublicKeyCredential(signed.value)))
            is Outcome.Failed -> failGet(GetCredentialUnknownException())
        }
    }

    private suspend fun password(request: ProviderGetCredentialRequest, caller: CredentialCaller, itemId: String) {
        if (request.credentialOptions.none { it is GetPasswordOption }) return failGet(GetCredentialUnknownException())
        val values = (container.credentialRepository.password(caller, itemId) as? Outcome.Ok)?.value
        val username = values?.username
        val password = values?.password
        if (username.isNullOrEmpty() || password.isNullOrEmpty()) return failGet(GetCredentialUnknownException())
        finishGet(GetCredentialResponse(PasswordCredential(username, password)))
    }
}
