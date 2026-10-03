package net.havenkeys.android.ui.shell

import androidx.annotation.StringRes
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.R
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.ui.kit.HavenIcon
import uniffi.havenkeys_mobile.ItemKind

/** The add sheet's tiles (spec §6.6). The identity is never one: there is one per account. */
enum class AddTile(@StringRes val label: Int, val icon: HavenIcon, val kind: ItemKind?) {
    LOGIN(R.string.vault_new_login, HavenIcon.Globe, ItemKind.LOGIN),
    NOTE(R.string.vault_new_note, HavenIcon.Note, ItemKind.SECURE_NOTE),
    CARD(R.string.vault_new_card, HavenIcon.Card, ItemKind.CARD),
    GENERATOR(R.string.add_generate, HavenIcon.Dice, null),
}

data class AddTileState(val tile: AddTile, val enabled: Boolean)

/** Creating needs the server; the generator works offline. */
fun tilesFor(online: Boolean): List<AddTileState> =
    AddTile.entries.map { AddTileState(it, enabled = online || it.kind == null) }

data class ShellUiState(
    val online: Boolean = false,
    /** Sync now is running: the top bar shows a ring instead of the button. */
    val syncing: Boolean = false,
    /** A failed sync's code, shown once as a toast; nothing from the vault. */
    val syncError: String? = null,
) {
    val addTiles: List<AddTileState> get() = tilesFor(online)
}

/** The shell's own state: whether writes can reach the server, Sync now, and the lock. Nothing from the vault. */
class ShellViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
) : ViewModel() {
    private val _state = MutableStateFlow(ShellUiState(online = events.online.value))
    val state: StateFlow<ShellUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch { events.online.collect { online -> _state.update { it.copy(online = online) } } }
        viewModelScope.launch {
            events.events.collect { event ->
                if (event is VaultEvent.Locked || event == VaultEvent.Removed || event == VaultEvent.SignedOut) {
                    _state.update { it.copy(syncing = false, syncError = null) }
                }
            }
        }
    }

    /**
     * The top bar's Sync now: the visible twin of pull to refresh, which
     * TalkBack cannot reach. The screens reload on Rust's `items_changed`.
     */
    fun sync() {
        if (_state.value.syncing) return
        _state.update { it.copy(syncing = true, syncError = null) }
        viewModelScope.launch {
            val result = accounts.syncNow()
            _state.update { it.copy(syncing = false, syncError = (result as? Outcome.Failed)?.code) }
        }
    }

    fun syncErrorShown() {
        _state.update { it.copy(syncError = null) }
    }

    fun lock() = vault.lock()
}
