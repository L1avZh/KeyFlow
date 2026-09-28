package app.keyflow.mobile.ui.screens.settings

import androidx.fragment.app.FragmentActivity
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.biometric.BiometricVaultUnlocker
import app.keyflow.mobile.data.AppPreferences
import app.keyflow.mobile.data.AppTheme
import app.keyflow.mobile.data.VaultRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class SettingsUiState(
    val theme: AppTheme = AppTheme.SYSTEM,
    val autoLockMinutes: Int = 5,
    val clipboardClearSeconds: Int = 30,
    val quickUnlockPreferenceOn: Boolean = false,
    val biometricsAvailable: Boolean = false,
    val quickUnlockActuallyEnabled: Boolean = false,
    val newPassword: String = "",
    val confirmPassword: String = "",
    val changePasswordMessage: String? = null,
    val exportedJson: String? = null,
    val errorMessage: String? = null,
)

class SettingsViewModel(
    private val repository: VaultRepository,
    private val preferences: AppPreferences,
    private val biometricUnlocker: BiometricVaultUnlocker,
) : ViewModel() {
    private val local = MutableStateFlow(
        SettingsUiState(
            biometricsAvailable = biometricUnlocker.canUseBiometrics(),
            quickUnlockActuallyEnabled = biometricUnlocker.isEnabled(),
        ),
    )

    val uiState: StateFlow<SettingsUiState> = combine(
        preferences.theme,
        preferences.autoLockMinutes,
        preferences.clipboardClearSeconds,
        preferences.quickUnlockEnabled,
        local,
    ) { theme, autoLock, clipboard, quickUnlockPref, localState ->
        localState.copy(
            theme = theme,
            autoLockMinutes = autoLock,
            clipboardClearSeconds = clipboard,
            quickUnlockPreferenceOn = quickUnlockPref,
        )
    }.stateIn(viewModelScope, kotlinx.coroutines.flow.SharingStarted.WhileSubscribed(5_000), local.value)

    fun setTheme(theme: AppTheme) = viewModelScope.launch { preferences.setTheme(theme) }

    fun setAutoLockMinutes(minutes: Int) = viewModelScope.launch { preferences.setAutoLockMinutes(minutes) }

    fun setClipboardClearSeconds(seconds: Int) = viewModelScope.launch { preferences.setClipboardClearSeconds(seconds) }

    fun enableQuickUnlock(activity: FragmentActivity, masterPassword: String) {
        viewModelScope.launch {
            biometricUnlocker.enable(activity, masterPassword).fold(
                onSuccess = {
                    preferences.setQuickUnlockEnabled(true)
                    local.update { it.copy(quickUnlockActuallyEnabled = true) }
                },
                onFailure = { e -> local.update { it.copy(errorMessage = e.message ?: "Could not enable quick unlock.") } },
            )
        }
    }

    fun disableQuickUnlock() {
        biometricUnlocker.disable()
        viewModelScope.launch { preferences.setQuickUnlockEnabled(false) }
        local.update { it.copy(quickUnlockActuallyEnabled = false) }
    }

    fun onNewPasswordChange(v: String) = local.update { it.copy(newPassword = v, changePasswordMessage = null) }
    fun onConfirmPasswordChange(v: String) = local.update { it.copy(confirmPassword = v, changePasswordMessage = null) }

    fun changeMasterPassword() {
        val s = local.value
        if (s.newPassword.codePointCount(0, s.newPassword.length) < repository.minMasterPasswordLength.toInt()) {
            local.update { it.copy(changePasswordMessage = "Use at least ${repository.minMasterPasswordLength} characters.") }
            return
        }
        if (s.newPassword != s.confirmPassword) {
            local.update { it.copy(changePasswordMessage = "Those passwords don't match.") }
            return
        }
        viewModelScope.launch {
            repository.changeMasterPassword(s.newPassword).fold(
                onSuccess = { local.update { it.copy(changePasswordMessage = "Master password changed.", newPassword = "", confirmPassword = "") } },
                onFailure = { e -> local.update { it.copy(changePasswordMessage = e.message) } },
            )
        }
    }

    fun exportJson() {
        viewModelScope.launch {
            repository.exportJsonPlaintext().fold(
                onSuccess = { json -> local.update { it.copy(exportedJson = json) } },
                onFailure = { e -> local.update { it.copy(errorMessage = e.message) } },
            )
        }
    }

    fun clearExportedJson() = local.update { it.copy(exportedJson = null) }

    fun lock() = repository.lock()
}
