package net.havenkeys.android.ui.trash

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
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
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.TrashSummary

/**
 * One row: the item's overview (title, username or a card's ending, kind,
 * whether it has a passkey) and when it was deleted, never a value
 * (spec 2026-10-08-trash §6.2). [daysLeft] is Rust's count; 0 means it
 * goes at the next sync.
 */
data class TrashRow(val summary: ItemSummary, val trashedAt: Long, val daysLeft: Int) {
    val id: String get() = summary.id
}

/**
 * [busy]: a restore, a delete or an empty is with the server. [errorCode]:
 * why the last action failed; [loadErrorCode]: why the list did not load,
 * cleared by the next load that works.
 */
data class TrashUiState(
    val rows: List<TrashRow> = emptyList(),
    val loading: Boolean = true,
    val busy: Boolean = false,
    val errorCode: String? = null,
    val loadErrorCode: String? = null,
)

/**
 * The Trash screen's list, newest first as Rust gives it. Every action
 * writes to the server first, then the list reloads; everything is dropped
 * on lock.
 */
class TrashViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(TrashUiState())
    val state: StateFlow<TrashUiState> = _state.asStateFlow()

    init {
        reload()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> _state.value = TrashUiState()
                    VaultEvent.ItemsChanged -> reload()
                    else -> Unit
                }
            }
        }
    }

    fun reload() {
        viewModelScope.launch {
            when (val r = vault.listTrash()) {
                is Outcome.Ok -> _state.update { s ->
                    s.copy(rows = r.value.map { it.toRow() }, loading = false, loadErrorCode = null)
                }
                is Outcome.Failed -> _state.update { it.copy(loading = false, loadErrorCode = r.code) }
            }
        }
    }

    /** [done] runs once the item is back in the vault. */
    fun restore(id: String, done: () -> Unit = {}) = act({ vault.restore(id) }, done)

    /** Deletes one item for good; [done] runs once it is gone. */
    fun purge(id: String, done: () -> Unit = {}) = act({ vault.purge(id) }, done)

    fun emptyTrash() = act({ vault.emptyTrash() }, {})

    private fun act(call: suspend () -> Outcome<*>, done: () -> Unit) {
        if (_state.value.busy) return
        _state.update { it.copy(busy = true, errorCode = null) }
        viewModelScope.launch {
            val r = call()
            val failed = (r as? Outcome.Failed)?.code
            // Restored or deleted on another device first: a sync removes it here.
            if (failed == CONFLICT) accounts.syncNow(fresh = true)
            _state.update { it.copy(busy = false, errorCode = failed) }
            if (r is Outcome.Ok) done()
            reload()
        }
    }
}

internal const val CONFLICT = "item_changed_elsewhere"

private fun TrashSummary.toRow() = TrashRow(summary = item, trashedAt = trashedAt, daysLeft = daysLeft.toInt())
