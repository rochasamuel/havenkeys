package net.havenkeys.android.data

import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import uniffi.havenkeys_mobile.VaultEvents

sealed interface VaultEvent {
    data class Locked(val reason: String) : VaultEvent
    data object Unlocked : VaultEvent
    data class Connectivity(val online: Boolean) : VaultEvent
    data object SignedOut : VaultEvent
    data object ItemsChanged : VaultEvent
    data object Removed : VaultEvent
}

/**
 * Rust's events, called on Rust's own event thread. Only posts to flows:
 * it must never call back into the vault.
 */
class VaultEventsHub : VaultEvents {
    private val _events = MutableSharedFlow<VaultEvent>(
        replay = 1,
        extraBufferCapacity = 16,
        onBufferOverflow = BufferOverflow.DROP_OLDEST,
    )
    val events: SharedFlow<VaultEvent> = _events

    private val _unlocked = MutableStateFlow(false)
    val unlocked: StateFlow<Boolean> = _unlocked

    private val _online = MutableStateFlow(false)
    val online: StateFlow<Boolean> = _online

    private val _accountDeleted = MutableStateFlow(false)

    /** The account was deleted: onboarding says so once, then calls [accountDeletedSeen]. */
    val accountDeleted: StateFlow<Boolean> = _accountDeleted

    override fun locked(reason: String) {
        _unlocked.value = false
        _online.value = false
        _events.tryEmit(VaultEvent.Locked(reason))
    }

    override fun unlocked() {
        _unlocked.value = true
        _events.tryEmit(VaultEvent.Unlocked)
    }

    override fun connectivity(online: Boolean) {
        _online.value = online
        _events.tryEmit(VaultEvent.Connectivity(online))
    }

    override fun signedOut() {
        _events.tryEmit(VaultEvent.SignedOut)
    }

    override fun itemsChanged() {
        _events.tryEmit(VaultEvent.ItemsChanged)
    }

    override fun removed() {
        _unlocked.value = false
        _events.tryEmit(VaultEvent.Removed)
    }

    /**
     * The phone's copy is already erased, exactly as after a removal, so every
     * listener wipes and the Keystore keys go on the same [VaultEvent.Removed].
     */
    override fun accountDeleted() {
        _accountDeleted.value = true
        removed()
    }

    fun accountDeletedSeen() {
        _accountDeleted.value = false
    }
}
