package net.havenkeys.android.ui.generator

import androidx.lifecycle.ViewModel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import net.havenkeys.android.data.Outcome
import net.havenkeys.android.data.SettingsRepository
import net.havenkeys.android.data.VaultRepository
import uniffi.havenkeys_mobile.GeneratorOptions

/** The options and the last strength; never the password itself (spec §9.4). */
data class GeneratorUiState(
    val options: GeneratorOptions = GeneratorViewModel.DEFAULT_OPTIONS,
    val entropyBits: Double? = null,
)

class GeneratorViewModel(
    private val vault: VaultRepository,
    private val settings: SettingsRepository,
) : ViewModel() {
    private val _state = MutableStateFlow(GeneratorUiState())
    val state: StateFlow<GeneratorUiState> = _state.asStateFlow()

    fun setOptions(options: GeneratorOptions) {
        val length = options.length.coerceIn(MIN_LENGTH, MAX_LENGTH)
        _state.update { it.copy(options = options.copy(length = length)) }
    }

    /** The caller keeps the password in its composition only. */
    suspend fun generate(): Outcome<String> = when (val r = vault.generate(_state.value.options)) {
        is Outcome.Ok -> {
            _state.update { it.copy(entropyBits = r.value.entropyBits) }
            Outcome.Ok(r.value.password)
        }
        is Outcome.Failed -> {
            _state.update { it.copy(entropyBits = null) }
            r
        }
    }

    suspend fun clipboardClearSeconds(): Int =
        (settings.get() as? Outcome.Ok)?.value?.clipboardClearSeconds?.toInt() ?: DEFAULT_CLIPBOARD_SECONDS

    companion object {
        // The core's generator::MIN_LENGTH and MAX_LENGTH.
        const val MIN_LENGTH = 8u
        const val MAX_LENGTH = 128u
        val DEFAULT_OPTIONS = GeneratorOptions(
            length = 24u,
            uppercase = true,
            lowercase = true,
            digits = true,
            symbols = true,
            avoidAmbiguous = false,
        )
        private const val DEFAULT_CLIPBOARD_SECONDS = 30
    }
}

/** The desktop's strength levels (apps/desktop/src/lib/format.ts). */
enum class Strength { WEAK, FAIR, STRONG, EXCELLENT }

@Suppress("MagicNumber")
fun strengthOf(bits: Double): Strength = when {
    bits < 50 -> Strength.WEAK
    bits < 75 -> Strength.FAIR
    bits < 110 -> Strength.STRONG
    else -> Strength.EXCELLENT
}
