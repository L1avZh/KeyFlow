package app.keyflow.mobile.ui.screens.generator

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.keyflow_mobile.PasswordOptionsRecord
import uniffi.keyflow_mobile.StrengthBandRecord
import uniffi.keyflow_mobile.estimateStrength
import uniffi.keyflow_mobile.generatePassword

data class GeneratorUiState(
    val password: String = "",
    val length: Int = 20,
    val uppercase: Boolean = true,
    val lowercase: Boolean = true,
    val digits: Boolean = true,
    val symbols: Boolean = true,
    val excludeAmbiguous: Boolean = true,
    val entropyBits: Double = 0.0,
    val strengthLabel: String = "",
)

/**
 * Generation and strength estimation both call straight into
 * `keyflow-core` (via the same UniFFI bridge the vault uses) rather than
 * reimplementing either in Kotlin — there is exactly one place in the
 * whole product, across desktop, browser extension, and Android, where
 * "how random/strong is this password" is decided.
 */
class GeneratorViewModel : ViewModel() {
    private val _uiState = MutableStateFlow(GeneratorUiState())
    val uiState: StateFlow<GeneratorUiState> = _uiState

    init {
        regenerate()
    }

    fun onLengthChange(length: Int) {
        _uiState.update { it.copy(length = length) }
        regenerate()
    }

    fun onToggle(uppercase: Boolean? = null, lowercase: Boolean? = null, digits: Boolean? = null, symbols: Boolean? = null, excludeAmbiguous: Boolean? = null) {
        _uiState.update {
            it.copy(
                uppercase = uppercase ?: it.uppercase,
                lowercase = lowercase ?: it.lowercase,
                digits = digits ?: it.digits,
                symbols = symbols ?: it.symbols,
                excludeAmbiguous = excludeAmbiguous ?: it.excludeAmbiguous,
            )
        }
        regenerate()
    }

    // Deliberately not dispatched to Dispatchers.Default/IO: unlike vault
    // unlock (Argon2id, genuinely tens of milliseconds of deliberate KDF
    // work), password generation and strength estimation are microsecond
    // CPU-only calls with no I/O — dispatching them would only add
    // scheduling overhead and complicate testing for no real benefit.
    fun regenerate() {
        val s = _uiState.value
        viewModelScope.launch {
            val options = PasswordOptionsRecord(
                length = s.length.toUInt(),
                uppercase = s.uppercase,
                lowercase = s.lowercase,
                digits = s.digits,
                symbols = s.symbols,
                excludeAmbiguous = s.excludeAmbiguous,
            )
            val result = runCatching { generatePassword(options) }
            result.onSuccess { password ->
                val strength = estimateStrength(password)
                _uiState.update {
                    it.copy(password = password, entropyBits = strength.entropyBits, strengthLabel = strength.band.label())
                }
            }
        }
    }
}

private fun StrengthBandRecord.label(): String = when (this) {
    StrengthBandRecord.VERY_WEAK -> "Very weak"
    StrengthBandRecord.WEAK -> "Weak"
    StrengthBandRecord.FAIR -> "Fair"
    StrengthBandRecord.STRONG -> "Strong"
    StrengthBandRecord.VERY_STRONG -> "Very strong"
}
