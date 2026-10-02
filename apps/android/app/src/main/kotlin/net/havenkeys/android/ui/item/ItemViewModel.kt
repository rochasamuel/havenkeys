package net.havenkeys.android.ui.item

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import net.havenkeys.android.data.clipboardClearSeconds
import uniffi.havenkeys_mobile.ItemView
import uniffi.havenkeys_mobile.TotpNow

/**
 * The item's overview: title, username, websites and which fields exist.
 * Revealed values and one-time codes are never kept here (spec §9.4).
 */
data class ItemUiState(val view: ItemView? = null, val errorCode: String? = null)

class ItemViewModel(
    private val vault: VaultRepository,
    private val settings: SettingsRepository,
    events: VaultEventsHub,
    private val id: String,
) : ViewModel() {
    private val _state = MutableStateFlow(ItemUiState())
    val state: StateFlow<ItemUiState> = _state.asStateFlow()

    init {
        load()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> _state.value = ItemUiState()
                    VaultEvent.ItemsChanged -> load()
                    else -> Unit
                }
            }
        }
    }

    /** One field's value, for the caller to show or copy; never stored here. */
    suspend fun reveal(key: String): Outcome<String> = vault.reveal(id, key)

    /** The current code, once per second while collected. */
    fun totpTicks(): Flow<Outcome<TotpNow>> = flow {
        while (true) {
            emit(vault.totp(id))
            delay(TICK_MS)
        }
    }

    /** Online only; the screen pops on success. */
    suspend fun delete(): Outcome<Unit> = vault.delete(id)

    suspend fun clipboardClearSeconds(): Int = settings.clipboardClearSeconds()

    private fun load() {
        viewModelScope.launch {
            _state.value = when (val view = vault.view(id)) {
                is Outcome.Ok -> ItemUiState(view = view.value)
                is Outcome.Failed -> ItemUiState(errorCode = view.code)
            }
        }
    }

    private companion object {
        const val TICK_MS = 1_000L
    }
}
