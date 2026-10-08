package net.havenkeys.android.ui.items

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import java.text.Collator
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
import net.havenkeys.android.ui.edit.tagKey
import uniffi.havenkeys_mobile.ItemSummary

/** Overviews only (Android spec §9.4): titles, usernames, websites, never a secret. */
data class ItemListUiState(
    val items: List<ItemSummary> = emptyList(),
    val refreshing: Boolean = false,
    val errorCode: String? = null,
    /** True until the first answer from Rust, so an empty list is not announced early. */
    val loading: Boolean = true,
)

/** How many items each category holds, for the Items tab. */
fun categoryCounts(items: List<ItemSummary>): Map<Category, Int> =
    Category.entries.associateWith { category -> items.count(category::keeps) }

/**
 * Every tag in use with how many items carry it, A–Z, for the Items tab.
 * Tags that differ only in case are one (two devices can each create a
 * spelling before they sync): the first spelling seen names it.
 */
fun tagCounts(items: List<ItemSummary>): List<Pair<String, Int>> {
    val order = readerOrder()
    val spelling = mutableMapOf<String, String>()
    val counts = mutableMapOf<String, Int>()
    items.forEach { item ->
        item.tags.distinctBy(::tagKey).forEach { tag ->
            val key = tagKey(tag)
            spelling.getOrPut(key) { tag }
            counts[key] = (counts[key] ?: 0) + 1
        }
    }
    return counts.map { (key, n) -> spelling.getValue(key) to n }.sortedWith { a, b -> order.compare(a.first, b.first) }
}

/** Whether [item] carries [tag], in any case. */
internal fun carries(item: ItemSummary, tag: String): Boolean = item.tags.any { tagKey(it) == tagKey(tag) }

/**
 * A–Z as a reader expects: accents and case do not split the alphabet
 * ("école" sorts with the e's, not after "z"). Text that only differs in
 * ways it ignores keeps a fixed order. Item titles, the tag list and the
 * editor's tag suggestions all use it, as the desktop's `localeCompare`.
 */
internal fun readerOrder(): Comparator<String> {
    val collator = Collator.getInstance().apply { strength = Collator.SECONDARY }
    return Comparator { a, b -> collator.compare(a, b).takeIf { it != 0 } ?: a.compareTo(b) }
}

/** Items A–Z by title, in [readerOrder]. */
internal fun alphabetical(items: List<ItemSummary>): List<ItemSummary> {
    val order = readerOrder()
    return items.sortedWith { a, b -> order.compare(a.title, b.title) }
}

/**
 * Whether a tag's list should close: the vault, as last read, has no item
 * carrying [tag] (its last item was untagged or deleted, here or by a
 * sync). Not while loading (the lock wipe resets to that) or after an error.
 */
internal fun tagGone(state: ItemListUiState, tag: String): Boolean =
    !state.loading && state.errorCode == null && state.items.none { carries(it, tag) }

/** The vault's items for the Items tab and a category list; each screen has its own. */
class ItemListViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(ItemListUiState())
    val state: StateFlow<ItemListUiState> = _state.asStateFlow()
    private var pending: Job? = null
    private var refreshJob: Job? = null

    init {
        load()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> wipe()
                    VaultEvent.Unlocked, VaultEvent.ItemsChanged -> load()
                    is VaultEvent.Connectivity -> Unit
                }
            }
        }
    }

    /** Pull to refresh: a sync, then the list again. */
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
            when (val result = vault.list()) {
                is Outcome.Ok -> _state.update {
                    it.copy(items = alphabetical(result.value), errorCode = null, loading = false)
                }
                is Outcome.Failed -> _state.update { it.copy(errorCode = result.code, loading = false) }
            }
        }
    }

    /** The lock wipe: nothing read from the vault stays here. */
    private fun wipe() {
        pending?.cancel()
        refreshJob?.cancel()
        _state.value = ItemListUiState()
    }
}
