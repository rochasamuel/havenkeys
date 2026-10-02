package net.havenkeys.android.ui.edit

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind

sealed interface EditTarget {
    data class Existing(val id: String) : EditTarget
    data class New(val kind: ItemKind) : EditTarget
}

/**
 * Which fields the item has and where the save stands. The draft itself is
 * the screen's (spec §9.4): it reaches here only as [save]'s argument and is
 * not kept. [generation] changes on every (re)load, and the screen starts a
 * fresh draft for each.
 */
data class EditUiState(
    val edit: ItemEdit? = null,
    val generation: Int = 0,
    val saving: Boolean = false,
    val errorCode: String? = null,
    val conflict: Boolean = false,
)

sealed interface EditResult {
    data class Saved(val id: String) : EditResult
}

class EditViewModel(
    private val vault: VaultRepository,
    private val accounts: AccountRepository,
    events: VaultEventsHub,
    private val target: EditTarget,
) : ViewModel() {
    private val _state = MutableStateFlow(EditUiState())
    val state: StateFlow<EditUiState> = _state.asStateFlow()
    private val _results = Channel<EditResult>(Channel.BUFFERED)
    val results: Flow<EditResult> = _results.receiveAsFlow()
    private var loads = 0

    init {
        load()
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked, VaultEvent.Removed, VaultEvent.SignedOut -> _state.value = EditUiState()
                    else -> Unit
                }
            }
        }
    }

    /** One current value, for the screen to put in its draft; never kept here. */
    suspend fun reveal(key: String): Outcome<String> = when (target) {
        is EditTarget.Existing -> vault.reveal(target.id, key)
        is EditTarget.New -> Outcome.Failed("not_found")
    }

    suspend fun generate(): Outcome<String> = when (val g = vault.generate(GENERATOR)) {
        is Outcome.Ok -> Outcome.Ok(g.value.password)
        is Outcome.Failed -> g
    }

    fun save(draft: ItemDraft) {
        if (_state.value.saving) return
        _state.update { it.copy(saving = true, errorCode = null) }
        viewModelScope.launch {
            val result = when (target) {
                is EditTarget.New -> vault.create(draft)
                is EditTarget.Existing -> when (val r = vault.update(target.id, draft)) {
                    is Outcome.Ok -> Outcome.Ok(target.id)
                    is Outcome.Failed -> r
                }
            }
            when {
                result is Outcome.Ok -> {
                    _state.update { it.copy(saving = false) }
                    _results.send(EditResult.Saved(result.value))
                }
                (result as Outcome.Failed).code == CONFLICT -> {
                    // Bring the other device's version in before offering to reload it.
                    accounts.syncNow()
                    _state.update { it.copy(saving = false, conflict = true) }
                }
                else -> _state.update { it.copy(saving = false, errorCode = result.code) }
            }
        }
    }

    fun reload() {
        _state.update { it.copy(conflict = false) }
        load()
    }

    private fun load() {
        val generation = loads++
        viewModelScope.launch {
            val edit = when (target) {
                is EditTarget.Existing -> vault.editable(target.id)
                is EditTarget.New -> vault.template(target.kind)
            }
            _state.value = when (edit) {
                is Outcome.Ok -> EditUiState(edit = edit.value, generation = generation)
                is Outcome.Failed -> EditUiState(generation = generation, errorCode = edit.code)
            }
        }
    }

    private companion object {
        const val CONFLICT = "item_changed_elsewhere"
        val GENERATOR = GeneratorOptions(24u, true, true, true, true, false)
    }
}
