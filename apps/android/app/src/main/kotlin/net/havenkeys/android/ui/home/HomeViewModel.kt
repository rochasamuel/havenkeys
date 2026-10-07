package net.havenkeys.android.ui.home

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
import net.havenkeys.android.ui.health.total
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary

/** The identity as Home shows it: its title and which kinds of detail it holds, never a value. */
data class IdentityCard(val id: String, val title: String, val parts: List<IdentityPart>)

/** Overviews and activity data only (spec §9); all of it is dropped on lock. */
data class HomeUiState(
    val identity: IdentityCard? = null,
    val recent: List<ItemSummary> = emptyList(),
    val frequent: List<ItemSummary> = emptyList(),
    val refreshing: Boolean = false,
    val errorCode: String? = null,
    /** True until the first answer from Rust, so empty lists are not announced early. */
    val loading: Boolean = true,
    /** Vault health's issue count for the Home card; null until known or when the check failed. */
    val healthTotal: Int? = null,
)

class HomeViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(HomeUiState())
    val state: StateFlow<HomeUiState> = _state.asStateFlow()
    private var pending: Job? = null
    private var refreshJob: Job? = null

    /** Home's groups settle in sequence the first time it shows, not on every return to the tab. */
    var settled = false

    init {
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.ItemsChanged -> load()
                    else -> Unit
                }
            }
        }
    }

    /** Each time Home shows: a copy or a fill since then changed "Frequently used" without an items event. */
    fun shown() = load()

    /** Pull to refresh: a sync, then the lists again. */
    fun refresh() {
        refreshJob = viewModelScope.launch {
            _state.update { it.copy(refreshing = true, errorCode = null) }
            val sync = accounts.syncNow()
            _state.update { it.copy(refreshing = false, errorCode = (sync as? Outcome.Failed)?.code) }
            if (sync is Outcome.Ok) load()
        }
    }

    private fun load() {
        pending?.cancel()
        pending = viewModelScope.launch {
            val recent = vault.recentlyCreated(LIST_SIZE)
            val frequent = vault.frequentlyUsed(LIST_SIZE)
            val identity = identityCard()
            val failed = (recent as? Outcome.Failed) ?: (frequent as? Outcome.Failed)
            _state.update {
                it.copy(
                    identity = identity,
                    recent = (recent as? Outcome.Ok)?.value.orEmpty(),
                    frequent = (frequent as? Outcome.Ok)?.value.orEmpty(),
                    errorCode = failed?.code,
                    loading = false,
                )
            }
            // After the lists: the check is slow on a large vault, and Home should not wait for it.
            val health = (vault.health() as? Outcome.Ok)?.value?.counts?.total()
            _state.update { it.copy(healthTotal = health) }
        }
    }

    /** The account's identity from its overview and the names of its filled fields; nothing is revealed. */
    private suspend fun identityCard(): IdentityCard? {
        val all = (vault.list() as? Outcome.Ok)?.value
        val identity = all?.firstOrNull { it.kind == ItemKind.IDENTITY } ?: return null
        val keys = (vault.view(identity.id) as? Outcome.Ok)?.value?.fields?.map { it.key }.orEmpty()
        return IdentityCard(identity.id, identity.title, IdentityPart.of(keys))
    }

    /** The lock wipe: no overview or activity data stays in this ViewModel. */
    private fun wipe() {
        pending?.cancel()
        refreshJob?.cancel()
        _state.value = HomeUiState()
    }

    companion object {
        /** Spec §6.5: the 6 most recently added and the 6 most used. */
        const val LIST_SIZE = 6
    }
}
