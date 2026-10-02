package net.havenkeys.android.credentials

import android.os.Build
import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.annotation.RequiresApi
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.credentials.CreatePublicKeyCredentialRequest
import androidx.credentials.CreatePublicKeyCredentialResponse
import androidx.credentials.exceptions.CreateCredentialCancellationException
import androidx.credentials.exceptions.CreateCredentialUnknownException
import androidx.credentials.exceptions.publickeycredential.CreatePublicKeyCredentialDomException
import androidx.credentials.exceptions.domerrors.InvalidStateError
import androidx.credentials.provider.PendingIntentHandler
import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.viewmodel.compose.viewModel
import kotlinx.coroutines.launch
import net.havenkeys.android.HavenApp
import net.havenkeys.android.R
import net.havenkeys.android.autofill.unlockThen
import net.havenkeys.android.security.hardenWindow
import net.havenkeys.android.ui.theme.HavenTheme

/**
 * "Save a passkey to HavenKeys?" (spec §8.2): unlock if needed, choose the
 * login, verify the user, then Rust creates, the server accepts, and only
 * then the site gets the passkey. Not exported.
 */
@RequiresApi(Build.VERSION_CODES.UPSIDE_DOWN_CAKE)
class PasskeyCreateActivity : FragmentActivity() {
    private val container get() = (application as HavenApp).container

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        hardenWindow()
        val provider = PendingIntentHandler.retrieveProviderCreateCredentialRequest(intent)
        val request = provider?.callingRequest as? CreatePublicKeyCredentialRequest
            ?: return failCreate(CreateCredentialUnknownException())
        val caller = provider.callingAppInfo.toCaller()
        unlockThen(container) { unlockedHere ->
            setContent {
                HavenTheme {
                    val vm = viewModel {
                        PasskeyCreateViewModel(
                            container.credentialRepository,
                            caller,
                            request.requestJson,
                            request.isConditional,
                        ).also { it.load() }
                    }
                    val state by vm.state.collectAsStateWithLifecycle()
                    LaunchedEffect(state.response) {
                        state.response?.let { finishCreate(CreatePublicKeyCredentialResponse(it)) }
                    }
                    LaunchedEffect(state.error) {
                        if (state.error == "locked") failCreate(CreateCredentialCancellationException())
                    }
                    PasskeyCreateScreen(
                        state = state,
                        onSelect = vm::select,
                        onSave = {
                            lifecycleScope.launch {
                                val subtitle = getString(R.string.passkey_verify_save, state.rpId)
                                if (userVerified(container, unlockedHere, subtitle)) vm.create()
                            }
                        },
                        onCancel = { failCreate(CreateCredentialCancellationException()) },
                        onClose = { failCreate(CreatePublicKeyCredentialDomException(InvalidStateError())) },
                    )
                }
            }
        }
    }
}
