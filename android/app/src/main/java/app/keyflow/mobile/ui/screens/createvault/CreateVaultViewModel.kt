package app.keyflow.mobile.ui.screens.createvault

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.data.VaultRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class CreateVaultUiState(
    val password: String = "",
    val confirmPassword: String = "",
    val isSubmitting: Boolean = false,
    val errorMessage: String? = null,
    val minLength: Int = 10,
)

class CreateVaultViewModel(private val repository: VaultRepository) : ViewModel() {
    private val _uiState = MutableStateFlow(CreateVaultUiState(minLength = repository.minMasterPasswordLength.toInt()))
    val uiState: StateFlow<CreateVaultUiState> = _uiState

    fun onPasswordChange(value: String) {
        _uiState.update { it.copy(password = value, errorMessage = null) }
    }

    fun onConfirmPasswordChange(value: String) {
        _uiState.update { it.copy(confirmPassword = value, errorMessage = null) }
    }

    fun submit(onSuccess: () -> Unit) {
        val state = _uiState.value
        // Client-side pre-flight only, using the same minLength the Rust
        // core reports (min_master_password_length()) rather than a
        // separately hardcoded number — the desktop app once shipped
        // with exactly that kind of drift between a UI check and the
        // backend's real check (see repo QA_REPORT.md, BUG-011).
        // codePointCount (not .length) matches Kotlin's own Unicode
        // scalar-value counting, the same class of fix.
        if (state.password.codePointCount(0, state.password.length) < state.minLength) {
            _uiState.update { it.copy(errorMessage = "Use at least ${it.minLength} characters.") }
            return
        }
        if (state.password != state.confirmPassword) {
            _uiState.update { it.copy(errorMessage = "Those passwords don't match.") }
            return
        }
        _uiState.update { it.copy(isSubmitting = true, errorMessage = null) }
        viewModelScope.launch {
            repository.createVault(state.password).fold(
                onSuccess = { onSuccess() },
                onFailure = { e -> _uiState.update { it.copy(isSubmitting = false, errorMessage = e.message) } },
            )
        }
    }
}
