package net.havenkeys.android.ui.pairing

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.PairingRequestView

/** The code and the request are not secrets; nothing secret passes through here. */
data class PairingUiState(
    val stage: Stage = Stage.SCANNING,
    val request: PairingRequestView? = null,
    val busy: Boolean = false,
    val errorCode: String? = null,
) {
    enum class Stage { SCANNING, CONFIRM, DONE, DENIED }
}

/**
 * Settings → Sign in a new device (spec 2026-10-03-phone-approved-sign-in
 * §3.2): scan, show who is asking, and approve only after the biometric
 * check passes.
 */
class PairingViewModel(private val accounts: AccountRepository) : ViewModel() {
    private val _state = MutableStateFlow(PairingUiState())
    val state: StateFlow<PairingUiState> = _state.asStateFlow()
    private val decoding = AtomicBoolean(false)

    /** The code that just failed: the camera keeps seeing it, and asking the server again would only rate-limit. */
    private var failedLink: String? = null

    fun onFrame(frame: LumaFrame) {
        if (_state.value.stage != PairingUiState.Stage.SCANNING || !decoding.compareAndSet(false, true)) {
            frame.bytes.fill(0)
            return
        }
        viewModelScope.launch {
            try {
                val link = (accounts.scanPairing(frame) as? Outcome.Ok)?.value ?: return@launch
                if (link == failedLink) return@launch
                when (val r = accounts.pairingRequest(link)) {
                    is Outcome.Ok -> {
                        failedLink = null
                        _state.value = PairingUiState(PairingUiState.Stage.CONFIRM, r.value)
                    }
                    is Outcome.Failed -> {
                        failedLink = link
                        _state.update { it.copy(errorCode = r.code) }
                    }
                }
            } finally {
                decoding.set(false)
            }
        }
    }

    /**
     * [verify] is the biometric prompt; false (cancelled, failed) approves nothing, silently.
     * With no [canVerify] (no biometric or screen lock set up) nothing is asked and the screen says why.
     */
    fun allow(canVerify: Boolean, verify: suspend () -> Boolean) {
        val request = _state.value.request
        if (request == null || _state.value.busy) return
        if (!canVerify) {
            _state.update { it.copy(errorCode = "biometric_unavailable") }
            return
        }
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch {
            if (!verify()) {
                _state.update { it.copy(busy = false) }
                return@launch
            }
            when (val r = accounts.approvePairing(request.link)) {
                is Outcome.Ok -> _state.value = PairingUiState(PairingUiState.Stage.DONE, request)
                is Outcome.Failed -> _state.update { it.copy(busy = false, errorCode = r.code) }
            }
        }
    }

    fun deny() {
        val request = _state.value.request ?: return
        if (_state.value.busy) return
        _state.update { it.copy(busy = true) }
        viewModelScope.launch {
            accounts.denyPairing(request.link)
            _state.value = PairingUiState(PairingUiState.Stage.DENIED, request)
        }
    }

    /** Back to the camera after an error or a finished approval. */
    fun scanAgain() {
        failedLink = null
        _state.value = PairingUiState()
    }
}
