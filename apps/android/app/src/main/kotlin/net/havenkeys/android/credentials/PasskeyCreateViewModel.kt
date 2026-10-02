package net.havenkeys.android.credentials

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import net.havenkeys.android.data.CredentialRepository
import net.havenkeys.android.data.Outcome
import uniffi.havenkeys_mobile.AutofillMatch
import uniffi.havenkeys_mobile.CredentialCaller

/**
 * `selected`: the login to hold the passkey; null makes a new login.
 * `response`: the registration JSON (public data) once the server accepted
 * the passkey, handed to Android exactly once.
 */
data class PasskeyCreateUiState(
    val loading: Boolean = true,
    val rpId: String = "",
    val userName: String = "",
    val homes: List<AutofillMatch> = emptyList(),
    val selected: String? = null,
    val excluded: Boolean = false,
    val busy: Boolean = false,
    val error: String? = null,
    val planFailed: Boolean = false,
    val response: String? = null,
)

/** "Save a passkey to HavenKeys?" Rust decides the homes and refuses what the caller may not do. */
class PasskeyCreateViewModel(
    private val repo: CredentialRepository,
    private val caller: CredentialCaller,
    private val requestJson: String,
    private val conditional: Boolean,
) : ViewModel() {
    private val _state = MutableStateFlow(PasskeyCreateUiState())
    val state: StateFlow<PasskeyCreateUiState> = _state.asStateFlow()

    fun load() {
        // A site's automatic upgrade would save without a tap here: never on Android.
        if (conditional) {
            _state.value = PasskeyCreateUiState(loading = false, error = "denied", planFailed = true)
            return
        }
        viewModelScope.launch {
            _state.value = when (val plan = repo.passkeyCreatePlan(caller, requestJson)) {
                is Outcome.Ok -> PasskeyCreateUiState(
                    loading = false,
                    rpId = plan.value.rpId,
                    userName = plan.value.userName,
                    homes = plan.value.homes,
                    selected = homeFor(plan.value.userName, plan.value.homes),
                    excluded = plan.value.excluded,
                )
                is Outcome.Failed -> PasskeyCreateUiState(loading = false, error = plan.code, planFailed = true)
            }
        }
    }

    fun select(itemId: String?) = _state.update {
        if (it.planFailed) it else it.copy(selected = itemId, error = null)
    }

    /** Call only after user verification passed. */
    fun create() {
        val s = _state.value
        val working = s.loading || s.busy
        val finished = s.excluded || s.response != null || s.planFailed
        if (working || finished || conditional) return
        _state.update { it.copy(busy = true, error = null) }
        viewModelScope.launch {
            when (val created = repo.passkeyCreate(caller, requestJson, s.selected)) {
                is Outcome.Ok -> _state.update { it.copy(busy = false, response = created.value) }
                is Outcome.Failed -> _state.update { it.copy(busy = false, error = created.code) }
            }
        }
    }
}

/** Only an account with the same username is preselected; otherwise the passkey makes a new login. */
private fun homeFor(userName: String, homes: List<AutofillMatch>): String? {
    val wanted = userName.trim().lowercase()
    if (wanted.isEmpty()) return null
    return homes.firstOrNull { it.username?.trim()?.lowercase() == wanted }?.id
}
