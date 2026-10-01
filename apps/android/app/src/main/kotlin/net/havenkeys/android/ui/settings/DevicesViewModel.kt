package net.havenkeys.android.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import uniffi.havenkeys_mobile.DeviceInfo

data class DevicesUiState(
    val devices: List<DeviceInfo> = emptyList(),
    val loading: Boolean = true,
    val errorCode: String? = null,
)

/** The account's devices, from the server; offline shows `error_offline`. */
class DevicesViewModel(private val accounts: AccountRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(DevicesUiState())
    val state: StateFlow<DevicesUiState> = _state.asStateFlow()
    private var pendingLoad: Job? = null

    init {
        load()
        viewModelScope.launch {
            events.events.collect { event ->
                if (event is VaultEvent.Connectivity && event.online) load()
            }
        }
    }

    fun revoke(id: String) {
        viewModelScope.launch {
            when (val r = accounts.revoke(id)) {
                is Outcome.Ok -> load()
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code) }
            }
        }
    }

    private fun load() {
        pendingLoad?.cancel()
        pendingLoad = viewModelScope.launch {
            _state.value = when (val r = accounts.devices()) {
                is Outcome.Ok -> DevicesUiState(devices = r.value, loading = false)
                is Outcome.Failed -> DevicesUiState(loading = false, errorCode = r.code)
            }
        }
    }
}
