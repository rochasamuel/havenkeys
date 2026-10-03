package net.havenkeys.android.ui.search

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
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
    /**
     * Bumped each time the ViewModel itself changes the query (a recent search tapped, the lock
     * wipe), so the field can tell those from the echo of its own typing.
     */
    val queryRevision: Int = 0,
)

/** Typed queries wait this long for the next keystroke, so fast typing runs one search, not several. */
private const val TYPING_DEBOUNCE_MS = 120L

class SearchViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _state = MutableStateFlow(SearchUiState())
    val state: StateFlow<SearchUiState> = _state.asStateFlow()
    private var pending: Job? = null
    private var recentsJob: Job? = null
    private var clearJob: Job? = null
    private var recordJob: Job? = null

    /** The query that produced the current results: what a tapped result records, not what is typed since. */
    private var resultsQuery = ""

    /** The field has taken focus once; coming back from an opened result must not raise the keyboard again. */
    private var focusedOnce = false

    /** True the first time only: the screen asks before it focuses the field. */
    fun takeFirstFocus(): Boolean = !focusedOnce.also { focusedOnce = true }

    init {
        loadRecents()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> _state.value.query
                        .takeIf { it.isNotBlank() }
                        ?.let { run(it, typed = false) }
                    else -> Unit
                }
            }
        }
    }

    /**
     * Live search as the user types, after a short pause in typing. Typing is activity for the
     * auto-lock; it records nothing. An empty field clears at once.
     */
    fun setQuery(query: String) {
        if (query == _state.value.query) return
        _state.update { it.copy(query = query, searched = false, errorCode = null) }
        if (query.isBlank()) {
            pending?.cancel()
            resultsQuery = ""
            _state.update { it.copy(results = emptyList()) }
            loadRecents()
        } else {
            run(query, typed = true)
        }
    }

    /** A recent search tapped: it runs again. It is recorded only if a result is then opened. */
    fun useRecent(query: String) {
        _state.update { it.copy(queryRevision = it.queryRevision + 1) }
        setQuery(query)
    }

    /** A result was opened: the query that produced the list it was tapped in becomes a recent search. */
    fun opened() {
        val query = resultsQuery.trim()
        if (query.isEmpty()) return
        recordJob = viewModelScope.launch { vault.recordSearch(query) }
    }

    fun clearRecents() {
        clearJob = viewModelScope.launch {
            if (vault.clearRecentSearches() is Outcome.Ok) _state.update { it.copy(recents = emptyList()) }
        }
    }

    private fun run(query: String, typed: Boolean) {
        pending?.cancel()
        pending = viewModelScope.launch {
            if (typed) {
                delay(TYPING_DEBOUNCE_MS)
                // Soft-keyboard typing does not reach `onUserInteraction`, and reads never count (Rust).
                vault.touch()
            }
            when (val result = vault.search(query)) {
                is Outcome.Ok -> {
                    resultsQuery = query
                    _state.update { it.copy(results = result.value, searched = true, errorCode = null) }
                }
                is Outcome.Failed -> {
                    resultsQuery = ""
                    _state.update { it.copy(results = emptyList(), errorCode = result.code) }
                }
            }
        }
    }

    private fun loadRecents() {
        recentsJob?.cancel()
        recentsJob = viewModelScope.launch {
            (vault.recentSearches() as? Outcome.Ok)?.let { recents ->
                _state.update { it.copy(recents = recents.value) }
            }
        }
    }

    /** The lock wipe: the query, the results and the recents go, and nothing in flight can bring them back. */
    private fun wipe() {
        pending?.cancel()
        recentsJob?.cancel()
        clearJob?.cancel()
        recordJob?.cancel()
        resultsQuery = ""
        focusedOnce = false
        _state.update { SearchUiState(queryRevision = it.queryRevision + 1) }
    }
}
