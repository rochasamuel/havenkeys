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
import net.havenkeys.android.ui.items.readerOrder
import net.havenkeys.android.ui.items.tagUse
import uniffi.havenkeys_mobile.GeneratorOptions
import uniffi.havenkeys_mobile.ItemDraft
import uniffi.havenkeys_mobile.ItemEdit
import uniffi.havenkeys_mobile.ItemKind
import uniffi.havenkeys_mobile.ItemSummary
import uniffi.havenkeys_mobile.LumaFrame

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
/**
 * [items]' tags, one spelling each, most used first, then A–Z. Where old
 * data holds a tag in several spellings, the one most items carry names it
 * (ties: the smallest by code point), as the spelling a new tag will take.
 */
internal fun tagsByUse(items: List<ItemSummary>): List<String> {
    val order = readerOrder()
    return tagUse(items)
        .sortedWith { a, b -> (b.second - a.second).takeIf { it != 0 } ?: order.compare(a.first, b.first) }
        .map { it.first }
}

data class EditUiState(
    val edit: ItemEdit? = null,
    val generation: Int = 0,
    val saving: Boolean = false,
    val errorCode: String? = null,
    val conflict: Boolean = false,
    /**
     * The other items' tags, one spelling each, most used first, then A–Z:
     * the editor's suggestions and the spelling a typed tag takes.
     */
    val vaultTags: List<String> = emptyList(),
) {
    // `edit` holds the username.
    override fun toString() = "EditUiState(…)"
}

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

    /** A camera frame for the one-time code field: an `otpauth://totp` link, or null. */
    suspend fun scanTotp(frame: LumaFrame): Outcome<String?> = vault.scanTotp(frame)

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
                    accounts.syncNow(fresh = true)
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
            // Suggestions are a convenience: without the list the editor offers none.
            // The item's own tags are left out, as Rust leaves the item out when it picks a spelling.
            val others = (vault.list() as? Outcome.Ok)?.value.orEmpty()
                .filter { target !is EditTarget.Existing || it.id != target.id }
            val vaultTags = tagsByUse(others)
            _state.value = when (edit) {
                is Outcome.Ok -> EditUiState(edit = edit.value, generation = generation, vaultTags = vaultTags)
                is Outcome.Failed -> EditUiState(generation = generation, errorCode = edit.code)
            }
        }
    }

    private companion object {
        const val CONFLICT = "item_changed_elsewhere"
        val GENERATOR = GeneratorOptions(24u, true, true, true, true, false)
    }
}
