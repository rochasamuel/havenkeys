package net.havenkeys.android.ui.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.AccountRepository
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultEvent
import net.havenkeys.android.data.VaultEventsHub
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.MobileSettings
import java.time.Instant
import java.time.format.DateTimeParseException
import java.time.temporal.ChronoUnit

/** [biometricEnrolled] is whether a bundle exists, never the bundle. */
data class SettingsUiState(
    val settings: MobileSettings? = null,
    val biometricEnrolled: Boolean = false,
    val errorCode: String? = null,
    /** The account's email, shown in "Type … to confirm". */
    val email: String? = null,
    val removing: Boolean = false,
    val removeErrorCode: String? = null,
    /** How many removals have failed: the dialog re-arms its confirm on each (an error message could repeat). */
    val removeFailures: Int = 0,
    val deleting: Boolean = false,
    val deleteErrorCode: String? = null,
    /** As [removeFailures], for "Delete account". */
    val deleteFailures: Int = 0,
    /** How many items are in the Trash, for its row; null until known. */
    val trashCount: Int? = null,
    /** Rust's plan status (for example "trialing"); null for an account without one. */
    val planStatus: String? = null,
    /** "full" or "frozen": Rust decides, this only displays. */
    val entitlement: String = "full",
    /** RFC 3339 end of the trial, when there is one. */
    val trialEndsAt: String? = null,
) {
    val frozen: Boolean get() = entitlement == "frozen"
}

/** Whole days left, rounded up, never below zero; null when [trialEndsAt] is unreadable. */
fun trialDaysLeft(trialEndsAt: String, now: Instant = Instant.now()): Int? {
    val end = try {
        Instant.parse(trialEndsAt)
    } catch (@Suppress("SwallowedException") e: DateTimeParseException) {
        return null
    }
    val seconds = ChronoUnit.SECONDS.between(now, end)
    val days = Math.floorDiv(seconds + SECONDS_PER_DAY - 1, SECONDS_PER_DAY)
    return days.coerceIn(0, Int.MAX_VALUE.toLong()).toInt()
}

private const val SECONDS_PER_DAY = 86_400L

@Suppress("TooManyFunctions") // one setter per setting, plus the account actions
class SettingsViewModel(
    private val settings: SettingsRepository,
    private val accounts: AccountRepository,
    private val vault: VaultRepository,
    events: VaultEventsHub,
    private val biometricEnrolled: () -> Boolean,
) : ViewModel() {
    private val _state = MutableStateFlow(SettingsUiState(biometricEnrolled = biometricEnrolled()))
    val state: StateFlow<SettingsUiState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val email = (vault.status() as? Outcome.Ok)?.value?.email
            when (val r = settings.get()) {
                is Outcome.Ok -> _state.update { it.copy(settings = r.value, email = email) }
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code, email = email) }
            }
        }
        loadPlan()
        loadTrashCount()
        viewModelScope.launch {
            events.events.collect {
                when (it) {
                    VaultEvent.ItemsChanged -> loadTrashCount()
                    VaultEvent.PlanChanged -> loadPlan()
                    else -> Unit
                }
            }
        }
    }

    /** A failed read keeps what is shown; Rust refuses the writes either way. */
    private fun loadPlan() {
        viewModelScope.launch {
            val s = (vault.status() as? Outcome.Ok)?.value ?: return@launch
            _state.update {
                it.copy(planStatus = s.planStatus, entitlement = s.entitlement, trialEndsAt = s.trialEndsAt)
            }
        }
    }

    /** A failed count leaves the row without one; the Trash screen says why. */
    private fun loadTrashCount() {
        viewModelScope.launch {
            val r = vault.listTrash()
            if (r is Outcome.Ok) _state.update { it.copy(trashCount = r.value.size) }
        }
    }

    fun setAutoLock(minutes: UInt) = save { it.copy(autoLockMinutes = minutes) }

    fun setClipboardSeconds(seconds: UInt) = save { it.copy(clipboardClearSeconds = seconds) }

    fun setLockOnScreenOff(on: Boolean) = save { it.copy(lockOnScreenOff = on) }

    fun setConfirmBeforeFilling(on: Boolean) = save { it.copy(confirmBeforeFilling = on) }

    fun setAssetLinks(on: Boolean) = save { it.copy(assetLinks = on) }

    /** Rust saves the whole record; the shown value changes only once it is accepted. */
    private fun save(change: (MobileSettings) -> MobileSettings) {
        val current = _state.value.settings ?: return
        val next = change(current)
        viewModelScope.launch {
            when (val r = settings.update(next)) {
                is Outcome.Ok -> _state.update { it.copy(settings = next, errorCode = null) }
                is Outcome.Failed -> _state.update { it.copy(errorCode = r.code) }
            }
        }
    }

    /**
     * After the biometric switch, with the enrollment's result when it was
     * turned on: a cancelled prompt says nothing, and the switch reads off.
     */
    fun biometricChanged(result: Outcome<Unit> = Outcome.Ok(Unit)) {
        val code = (result as? Outcome.Failed)?.code?.takeIf { it != CANCELLED }
        _state.update { it.copy(biometricEnrolled = biometricEnrolled(), errorCode = code) }
    }

    /** The container deletes the biometric key and bundle on the `signedOut` event. */
    fun signOut() {
        viewModelScope.launch {
            val r = accounts.signOut()
            if (r is Outcome.Failed) _state.update { it.copy(errorCode = r.code) }
        }
    }

    /** Rust checks [confirmation] against the account's email. */
    fun removeDevice(confirmation: String) {
        if (_state.value.removing) return
        _state.update { it.copy(removing = true, removeErrorCode = null) }
        viewModelScope.launch {
            val r = accounts.removeDevice(confirmation)
            val code = (r as? Outcome.Failed)?.code
            _state.update {
                it.copy(
                    removing = false,
                    removeErrorCode = code,
                    removeFailures = if (code != null) it.removeFailures + 1 else it.removeFailures,
                )
            }
        }
    }

    /** Rust checks [confirmation] and [masterPassword]; neither is kept here. */
    fun deleteAccount(confirmation: String, masterPassword: String) {
        if (_state.value.deleting) return
        _state.update { it.copy(deleting = true, deleteErrorCode = null) }
        viewModelScope.launch {
            val r = accounts.deleteAccount(confirmation, masterPassword)
            val code = (r as? Outcome.Failed)?.code
            _state.update {
                it.copy(
                    deleting = false,
                    deleteErrorCode = code,
                    deleteFailures = if (code != null) it.deleteFailures + 1 else it.deleteFailures,
                )
            }
        }
    }

    /** A closed removal or deletion dialog forgets its error. */
    fun clearAccountErrors() {
        _state.update { it.copy(removeErrorCode = null, deleteErrorCode = null) }
    }

    companion object {
        /** The core's AUTO_LOCK_CHOICES; 0 is never. */
        val AUTO_LOCK_CHOICES = listOf(0u, 5u, 15u, 30u, 60u)

        /** The desktop's choices, inside the core's 10–300 s. */
        val CLIPBOARD_CHOICES = listOf(10u, 20u, 30u, 60u, 90u, 120u)

        private const val CANCELLED = "cancelled"
    }
}
