package net.havenkeys.android.ui.nav

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.LockState

enum class Start { ONBOARDING, UNLOCK, VAULT }

class RootViewModel(private val vault: VaultRepository, events: VaultEventsHub) : ViewModel() {
    private val _start = MutableStateFlow<Start?>(null)
    val start: StateFlow<Start?> = _start.asStateFlow()

    // A channel, not a SharedFlow: a lock that happens while the activity is
    // being recreated must still wipe the screens when the new one collects.
    private val signals = Channel<Start>(Channel.BUFFERED)

    /** Where the lock wipe goes: [Start.UNLOCK] on a lock, [Start.ONBOARDING] when the vault is removed. */
    val lockedSignal: Flow<Start> = signals.receiveAsFlow()

    init {
        viewModelScope.launch { _start.value = current() }
        viewModelScope.launch {
            events.events.collect { event ->
                when (event) {
                    is VaultEvent.Locked -> signals.send(Start.UNLOCK)
                    VaultEvent.Removed -> signals.send(Start.ONBOARDING)
                    else -> Unit
                }
            }
        }
    }

    /** The screen the vault's state calls for now; onboarding asks once it is done. */
    suspend fun current(): Start = when (val status = vault.status()) {
        is Outcome.Ok -> when {
            !status.value.vaultExists -> Start.ONBOARDING
            status.value.state == LockState.UNLOCKED -> Start.VAULT
            else -> Start.UNLOCK
        }
        // Unknown state: the locked screen is the safe one.
        is Outcome.Failed -> Start.UNLOCK
    }
}
