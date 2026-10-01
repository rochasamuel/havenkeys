package net.havenkeys.android.ui.unlock

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultRepository

/** Holds no secret: the password and the bundle go straight to Rust. */
data class UnlockUiState(
    val busy: Boolean = false,
    val errorCode: String? = null,
    val offerBiometric: Boolean = false,
    val unlocked: Boolean = false,
)

class UnlockViewModel(
    private val vault: VaultRepository,
    biometricAvailable: Boolean,
    hasBundle: () -> Boolean,
    private val deleteBundle: () -> Unit,
) : ViewModel() {
    private val _state = MutableStateFlow(UnlockUiState(offerBiometric = biometricAvailable && hasBundle()))
    val state: StateFlow<UnlockUiState> = _state.asStateFlow()

    fun unlockPassword(password: String) {
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch { finish(vault.unlockPassword(password)) }
    }

    /** [bundle] is zeroed when the call ends, however it ends. */
    fun unlockWithBundle(bundle: ByteArray, bootCount: Long) {
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch {
            val result = vault.unlockWithBundle(bundle, bootCount)
            if (result is Outcome.Failed) {
                // Too old, from another boot or tampered (spec §5.2): only the password helps now.
                if (result.code == BUNDLE_REFUSED) bundleUnusable() else hideBiometric()
            }
            finish(result)
        }.invokeOnCompletion { bundle.fill(0) }
    }

    /** The bundle is gone, stale or its key invalidated: delete it and ask for the password. */
    fun bundleUnusable() {
        deleteBundle()
        hideBiometric()
    }

    private fun hideBiometric() = _state.update { it.copy(offerBiometric = false) }

    private fun finish(result: Outcome<*>) = when (result) {
        is Outcome.Ok -> _state.update { it.copy(busy = false, unlocked = true) }
        is Outcome.Failed -> _state.update { it.copy(busy = false, errorCode = result.code) }
    }

    private companion object {
        const val BUNDLE_REFUSED = "bundle_refused"
    }
}
