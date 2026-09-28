package app.keyflow.mobile.ui.screens.vaultlist

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.data.VaultRepository
import app.keyflow.mobile.data.VaultUiState
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch
import uniffi.keyflow_mobile.CredentialRecord

data class VaultListUiState(
    val allCredentials: List<CredentialRecord> = emptyList(),
    val query: String = "",
    val favoritesOnly: Boolean = false,
) {
    val visibleCredentials: List<CredentialRecord>
        get() = allCredentials
            .filter { !favoritesOnly || it.favorite }
            .filter {
                query.isBlank() ||
                    it.name.contains(query, ignoreCase = true) ||
                    it.username.contains(query, ignoreCase = true) ||
                    it.url.contains(query, ignoreCase = true)
            }
            .sortedByDescending { it.favorite }
}

class VaultListViewModel(private val repository: VaultRepository) : ViewModel() {
    private val query = MutableStateFlow("")
    private val favoritesOnly = MutableStateFlow(false)

    val uiState: StateFlow<VaultListUiState> = combine(repository.state, query, favoritesOnly) { vaultState, q, favOnly ->
        val creds = (vaultState as? VaultUiState.Unlocked)?.credentials.orEmpty()
        VaultListUiState(allCredentials = creds, query = q, favoritesOnly = favOnly)
    }.stateIn(viewModelScope, kotlinx.coroutines.flow.SharingStarted.WhileSubscribed(5_000), VaultListUiState())

    fun onQueryChange(value: String) {
        query.value = value
    }

    fun onToggleFavoritesOnly() {
        favoritesOnly.value = !favoritesOnly.value
    }

    fun toggleFavorite(credential: CredentialRecord) {
        viewModelScope.launch {
            repository.updateCredential(credential.copy(favorite = !credential.favorite))
        }
    }

    fun deleteCredential(id: String) {
        viewModelScope.launch { repository.deleteCredential(id) }
    }

    fun lock() {
        repository.lock()
    }
}
