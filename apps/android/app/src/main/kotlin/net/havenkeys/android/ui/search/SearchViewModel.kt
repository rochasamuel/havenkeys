package net.havenkeys.android.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.ItemSummary

/**
 * The search screen's state. The query lives here, in memory, and nowhere
 * else: never in a route, saved instance state or a log (spec §9).
 */
data class SearchUiState(
    val query: String = "",
    /** From Rust's per-device activity slot, newest first. */
    val recents: List<String> = emptyList(),
    val results: List<ItemSummary> = emptyList(),
    /** A search for the current query has answered, so "No matches" may show. */
    val searched: Boolean = false,
    val errorCode: String? = null,
)

class SearchViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()
    private var pending: Job? = null

    init {
        loadRecents()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> _state.value.query
                        .takeIf { it.isNotBlank() }
                        ?.let(::run)
                    else -> Unit
                }
            }
        }
    }

    /** Live search as the user types. Typing is activity for the auto-lock; it records nothing. */
    fun setQuery(query: String) {
        if (query == _state.value.query) return
        // Soft-keyboard typing does not reach `onUserInteraction`, and reads never count (Rust).
        vault.touch()
        _state.update { it.copy(query = query, searched = false, errorCode = null) }
        if (query.isBlank()) {
            pending?.cancel()
            _state.update { it.copy(results = emptyList()) }
            loadRecents()
        } else {
            run(query)
        }
    }

    /** A recent search tapped: it runs again. It is recorded only if a result is then opened. */
    fun useRecent(query: String) = setQuery(query)

    /** A result was opened: the current query becomes a recent search (spec §6.3). */
    fun opened() {
        val query = _state.value.query.trim()
        if (query.isEmpty()) return
        viewModelScope.launch { vault.recordSearch(query) }
    }

    fun clearRecents() {
        viewModelScope.launch {
            if (vault.clearRecentSearches() is Outcome.Ok) _state.update { it.copy(recents = emptyList()) }
        }
    }

    private fun run(query: String) {
        pending?.cancel()
        pending = viewModelScope.launch {
            when (val result = vault.search(query)) {
                is Outcome.Ok -> _state.update { it.copy(results = result.value, searched = true, errorCode = null) }
                is Outcome.Failed -> _state.update { it.copy(results = emptyList(), errorCode = result.code) }
            }
        }
    }

    private fun loadRecents() {
        viewModelScope.launch {
            (vault.recentSearches() as? Outcome.Ok)?.let { recents ->
                _state.update { it.copy(recents = recents.value) }
            }
        }
    }

    /** The lock wipe: the query, the results and the recents go. */
    private fun wipe() {
        pending?.cancel()
        _state.value = SearchUiState()
    }
}
