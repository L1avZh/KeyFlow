package app.keyflow.mobile.ui.screens.unlock

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.biometric.BiometricVaultUnlocker
import app.keyflow.mobile.data.VaultRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class UnlockUiState(
    val password: String = "",
    val isSubmitting: Boolean = false,
    val errorMessage: String? = null,
)

class UnlockViewModel(
    private val repository: VaultRepository,
    private val biometricUnlocker: BiometricVaultUnlocker,
) : ViewModel() {
    private val _uiState = MutableStateFlow(UnlockUiState())
    val uiState: StateFlow<UnlockUiState> = _uiState

    val canOfferBiometrics: Boolean
        get() = biometricUnlocker.isEnabled() && biometricUnlocker.canUseBiometrics()

    fun onPasswordChange(value: String) {
        _uiState.update { it.copy(password = value, errorMessage = null) }
    }

    fun unlockWithPassword(onSuccess: suspend () -> Unit) {
        val password = _uiState.value.password
        if (password.isEmpty()) return
        _uiState.update { it.copy(isSubmitting = true, errorMessage = null) }
        viewModelScope.launch {
            val result = repository.unlock(password)
            result.onSuccess { onSuccess() }
            result.onFailure { e -> _uiState.update { it.copy(isSubmitting = false, errorMessage = e.message) } }
        }
    }

    fun unlockWithBiometrics(activity: FragmentActivity, onSuccess: suspend () -> Unit) {
        _uiState.update { it.copy(isSubmitting = true, errorMessage = null) }
        viewModelScope.launch {
            val biometricResult = biometricUnlocker.unlock(activity)
            val password = biometricResult.getOrElse { e ->
                _uiState.update { it.copy(isSubmitting = false, errorMessage = e.message ?: "Biometric unlock failed.") }
                return@launch
            }
            val result = repository.unlock(password)
            result.onSuccess { onSuccess() }
            result.onFailure { e -> _uiState.update { it.copy(isSubmitting = false, errorMessage = e.message) } }
        }
    }
}
