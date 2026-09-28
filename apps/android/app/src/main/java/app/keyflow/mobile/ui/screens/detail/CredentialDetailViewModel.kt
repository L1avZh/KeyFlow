package app.keyflow.mobile.ui.screens.detail

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.data.VaultUiState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.keyflow_mobile.NewCredential

data class CredentialDetailUiState(
    val id: String? = null,
    val name: String = "",
    val url: String = "",
    val username: String = "",
    val password: String = "",
    val notes: String = "",
    val favorite: Boolean = false,
    val isLoading: Boolean = true,
    val isSaving: Boolean = false,
    val errorMessage: String? = null,
    val saved: Boolean = false,
    val deleted: Boolean = false,
)

class CredentialDetailViewModel(private val repository: VaultRepository, credentialId: String?) : ViewModel() {
    private val _uiState = MutableStateFlow(CredentialDetailUiState(id = credentialId, isLoading = credentialId != null))
    val uiState: StateFlow<CredentialDetailUiState> = _uiState

    init {
        val id = credentialId
        if (id == null) {
            _uiState.update { it.copy(isLoading = false) }
        } else {
            val current = (repository.state.value as? VaultUiState.Unlocked)?.credentials?.find { it.id == id }
            if (current != null) {
                _uiState.update {
                    it.copy(
                        name = current.name,
                        url = current.url,
                        username = current.username,
                        password = current.password,
                        notes = current.notes,
                        favorite = current.favorite,
                        isLoading = false,
                    )
                }
            } else {
                _uiState.update { it.copy(isLoading = false, errorMessage = "This login could not be found.") }
            }
        }
    }

    fun onNameChange(v: String) = _uiState.update { it.copy(name = v) }
    fun onUrlChange(v: String) = _uiState.update { it.copy(url = v) }
    fun onUsernameChange(v: String) = _uiState.update { it.copy(username = v) }
    fun onPasswordChange(v: String) = _uiState.update { it.copy(password = v) }
    fun onNotesChange(v: String) = _uiState.update { it.copy(notes = v) }
    fun onFavoriteChange(v: Boolean) = _uiState.update { it.copy(favorite = v) }

    fun save() {
        val s = _uiState.value
        if (s.name.isBlank()) {
            _uiState.update { it.copy(errorMessage = "Give this login a name.") }
            return
        }
        _uiState.update { it.copy(isSaving = true, errorMessage = null) }
        viewModelScope.launch {
            val input = NewCredential(
                name = s.name,
                url = s.url,
                username = s.username,
                password = s.password,
                notes = s.notes,
                tags = emptyList(),
                favorite = s.favorite,
            )
            val result = if (s.id == null) {
                repository.addCredential(input)
            } else {
                val existing = (repository.state.value as? VaultUiState.Unlocked)?.credentials?.find { it.id == s.id }
                if (existing == null) {
                    _uiState.update { it.copy(isSaving = false, errorMessage = "This login could not be found.") }
                    return@launch
                }
                repository.updateCredential(
                    existing.copy(
                        name = s.name,
                        url = s.url,
                        username = s.username,
                        password = s.password,
                        notes = s.notes,
                        favorite = s.favorite,
                    ),
                ).map { s.id }
            }
            result.fold(
                onSuccess = { _uiState.update { it.copy(isSaving = false, saved = true) } },
                onFailure = { e -> _uiState.update { it.copy(isSaving = false, errorMessage = e.message) } },
            )
        }
    }

    fun delete() {
        val id = _uiState.value.id ?: return
        viewModelScope.launch {
            repository.deleteCredential(id).onSuccess {
                _uiState.update { it.copy(deleted = true) }
            }
        }
    }
}
