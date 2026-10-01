package net.havenkeys.android.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.MobileSettings

/** [biometricEnrolled] is whether a bundle exists, never the bundle. */
data class SettingsUiState(
    val settings: MobileSettings? = null,
    val biometricEnrolled: Boolean = false,
    val errorCode: String? = null,
    /** The account's email, shown in "Type … to confirm". */
    val email: String? = null,
    val removing: Boolean = false,
    val removeErrorCode: String? = null,
)

class SettingsViewModel(
    private val settings: SettingsRepository,
    private val accounts: AccountRepository,
    private val vault: VaultRepository,
    private val biometricEnrolled: () -> Boolean,
) : ViewModel() {
    private val _state = MutableStateFlow(SettingsUiState(biometricEnrolled = biometricEnrolled()))
    val state: StateFlow<SettingsUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val email = (vault.status() as? Outcome.Ok)?.value?.email
            when (val r = settings.get()) {
                is Outcome.Ok -> _state.update { it.copy(settings = r.value, email = email) }
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code, email = email) }
            }
        }
    }

    fun setAutoLock(minutes: UInt) = save { it.copy(autoLockMinutes = minutes) }

    fun setClipboardSeconds(seconds: UInt) = save { it.copy(clipboardClearSeconds = seconds) }

    fun setLockOnScreenOff(on: Boolean) = save { it.copy(lockOnScreenOff = on) }

    fun setConfirmBeforeFilling(on: Boolean) = save { it.copy(confirmBeforeFilling = on) }

    fun setAssetLinks(on: Boolean) = save { it.copy(assetLinks = on) }

    /** Rust saves the whole record; the shown value changes only once it is accepted. */
    private fun save(change: (MobileSettings) -> MobileSettings) {
        val current = _state.value.settings ?: return
        val next = change(current)
        viewModelScope.launch {
            when (val r = settings.update(next)) {
                is Outcome.Ok -> _state.update { it.copy(settings = next, errorCode = null) }
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code) }
            }
        }
    }

    /**
     * After the biometric switch, with the enrollment's result when it was
     * turned on: a cancelled prompt says nothing, and the switch reads off.
     */
    fun biometricChanged(result: Outcome<Unit> = Outcome.Ok(Unit)) {
        val code = (result as? Outcome.Failed)?.code?.takeIf { it != CANCELLED }
        _state.update { it.copy(biometricEnrolled = biometricEnrolled(), errorCode = code) }
    }

    /** The container deletes the biometric key and bundle on the `signedOut` event. */
    fun signOut() {
        viewModelScope.launch {
            val r = accounts.signOut()
            if (r is Outcome.Failed) _state.update { it.copy(errorCode = r.code) }
        }
    }

    /** Rust checks [confirmation] against the account's email. */
    fun removeDevice(confirmation: String) {
        if (_state.value.removing) return
        _state.update { it.copy(removing = true, removeErrorCode = null) }
        viewModelScope.launch {
            val r = accounts.removeDevice(confirmation)
            _state.update { it.copy(removing = false, removeErrorCode = (r as? Outcome.Failed)?.code) }
        }
    }

    fun clearRemoveError() {
        _state.update { it.copy(removeErrorCode = null) }
    }

    companion object {
        /** The core's AUTO_LOCK_CHOICES; 0 is never. */
        val AUTO_LOCK_CHOICES = listOf(0u, 5u, 15u, 30u, 60u)

        /** The desktop's choices, inside the core's 10–300 s. */
        val CLIPBOARD_CHOICES = listOf(10u, 20u, 30u, 60u, 90u, 120u)

        private const val CANCELLED = "cancelled"
    }
}
