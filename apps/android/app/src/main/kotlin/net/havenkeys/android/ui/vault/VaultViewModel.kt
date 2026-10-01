package net.havenkeys.android.ui.vault

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
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

enum class Filter { ALL, LOGINS, NOTES, CARDS, IDENTITIES, PASSKEYS }

/** Overviews only (spec §9.4): titles, usernames and websites, never a secret. */
data class VaultUiState(
    val items: List<ItemSummary> = emptyList(),
    val filter: Filter = Filter.ALL,
    val query: String = "",
    val refreshing: Boolean = false,
    val online: Boolean = false,
    val errorCode: String? = null,
    /** True until the first answer from Rust, so an empty vault is not announced early. */
    val loading: Boolean = true,
)

class VaultViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(VaultUiState(online = events.online.value))
    val state: StateFlow<VaultUiState> = _state.asStateFlow()

    /** What Rust returned for the current query, before the filter. */
    private var loaded: List<ItemSummary> = emptyList()
    private var pendingLoad: Job? = null

    init {
        reload()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.Unlocked, VaultEvent.ItemsChanged -> reload()
                    is VaultEvent.Connectivity -> _state.update { it.copy(online = event.online) }
                }
            }
        }
    }

    fun setQuery(query: String) {
        // Soft-keyboard typing does not reach `onUserInteraction`, and loads
        // never count as activity (Rust), so typing is reported here.
        vault.touch()
        _state.update { it.copy(query = query) }
        reload()
    }

    fun setFilter(filter: Filter) {
        _state.update { it.copy(filter = filter, items = loaded.filter(filter::keeps)) }
    }

    fun refresh() {
        viewModelScope.launch {
            _state.update { it.copy(refreshing = true, errorCode = null) }
            val sync = accounts.syncNow()
            _state.update { it.copy(refreshing = false, errorCode = (sync as? Outcome.Failed)?.code) }
            if (sync is Outcome.Ok) reload()
        }
    }

    private fun reload() {
        pendingLoad?.cancel()
        pendingLoad = viewModelScope.launch {
            val query = _state.value.query
            val result = if (query.isBlank()) vault.list() else vault.search(query)
            when (result) {
                is Outcome.Ok -> {
                    loaded = result.value
                    _state.update {
                        it.copy(items = loaded.filter(it.filter::keeps), errorCode = null, loading = false)
                    }
                }
                is Outcome.Failed -> _state.update { it.copy(errorCode = result.code, loading = false) }
            }
        }
    }

    /** The lock wipe: nothing read from the vault stays in this ViewModel. */
    private fun wipe() {
        pendingLoad?.cancel()
        loaded = emptyList()
        _state.update { it.copy(items = emptyList(), query = "") }
    }
}

internal fun Filter.keeps(item: ItemSummary): Boolean = when (this) {
    Filter.ALL -> true
    Filter.LOGINS -> item.kind == ItemKind.LOGIN
    Filter.NOTES -> item.kind == ItemKind.SECURE_NOTE
    Filter.CARDS -> item.kind == ItemKind.CARD
    Filter.IDENTITIES -> item.kind == ItemKind.IDENTITY
    Filter.PASSKEYS -> item.kind == ItemKind.LOGIN && item.hasPasskey
}
