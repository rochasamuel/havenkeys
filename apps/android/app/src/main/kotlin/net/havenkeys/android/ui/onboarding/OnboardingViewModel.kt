package net.havenkeys.android.ui.onboarding

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
import uniffi.havenkeys_mobile.InvitePreview
import uniffi.havenkeys_mobile.KitPreview
import uniffi.havenkeys_mobile.LumaFrame
import uniffi.havenkeys_mobile.Status

/** [preview] is the scanned kit's address and server only: Rust keeps its Secret Key. */
data class OnboardingUiState(
    val mode: Mode = Mode.CHOOSE,
    val preview: KitPreview? = null,
    /** The typed setup code's address and server; never the secret in it. */
    val invitePreview: InvitePreview? = null,
    val busy: Boolean = false,
    val errorCode: String? = null,
    val done: Boolean = false,
) {
    enum class Mode { CHOOSE, SCAN, TYPE, INVITE, PASSWORD }
}

/** Holds no secret: passwords, the Secret Key and the invite go straight from the text field to Rust. */
class OnboardingViewModel(private val accounts: AccountRepository) : ViewModel() {
    private val _state = MutableStateFlow(OnboardingUiState())
    val state: StateFlow<OnboardingUiState> = _state.asStateFlow()
    private val decoding = AtomicBoolean(false)

    fun choose(mode: OnboardingUiState.Mode) = _state.update { it.copy(mode = mode, errorCode = null) }

    fun back() {
        accounts.forgetKit()
        _state.value = OnboardingUiState()
    }

    /** Called from the camera thread. One frame is decoded at a time; the others are dropped. */
    fun onFrame(frame: LumaFrame) {
        if (_state.value.mode != OnboardingUiState.Mode.SCAN || !decoding.compareAndSet(false, true)) {
            wipe(frame)
            return
        }
        viewModelScope.launch {
            try {
                when (val result = accounts.scanKit(frame)) {
                    is Outcome.Ok -> result.value?.let(::showKit)
                    is Outcome.Failed -> _state.update { it.copy(errorCode = result.code) }
                }
            } finally {
                wipe(frame)
                decoding.set(false)
            }
        }
    }

    fun signInWithKit(password: String) = submit { accounts.signInWithKit(password) }

    fun signIn(server: String, email: String, password: String, secretKey: String) =
        submit { accounts.signIn(server.trim(), email.trim(), password, secretKey.trim()) }

    fun activate(invite: String, password: String) = submit { accounts.activate(invite.trim(), password) }

    /** Only a value that looks like a setup code is sent to Rust; anything else clears the preview. */
    fun previewInvite(invite: String) {
        val value = invite.trim()
        if (!value.startsWith(INVITE_PREFIX)) {
            _state.update { it.copy(invitePreview = null) }
            return
        }
        viewModelScope.launch {
            val preview = (accounts.previewInvite(value) as? Outcome.Ok)?.value
            _state.update { it.copy(invitePreview = preview) }
        }
    }

    private fun showKit(preview: KitPreview) {
        var shown = false
        _state.update {
            shown = it.mode == OnboardingUiState.Mode.SCAN
            if (shown) it.copy(mode = OnboardingUiState.Mode.PASSWORD, preview = preview, errorCode = null) else it
        }
        // The user left the scan while this frame was decoding: Rust must not keep the kit.
        if (!shown) accounts.forgetKit()
    }

    private fun submit(call: suspend () -> Outcome<Status>) {
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch {
            when (val result = call()) {
                is Outcome.Ok -> _state.update { it.copy(busy = false, done = true) }
                is Outcome.Failed -> _state.update { it.copy(busy = false, errorCode = result.code) }
            }
        }
    }

    // The frame is a picture of the kit, Secret Key included; Rust has its own copy.
    private fun wipe(frame: LumaFrame) = frame.bytes.fill(0)

    private companion object {
        const val INVITE_PREFIX = "HKINV1-"
    }
}
