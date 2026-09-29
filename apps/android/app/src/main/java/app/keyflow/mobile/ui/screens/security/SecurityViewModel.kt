package app.keyflow.mobile.ui.screens.security

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.keyflow.mobile.data.VaultRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.keyflow_mobile.CredentialRecord
import uniffi.keyflow_mobile.SecurityOverviewRecord

enum class SecurityTab { OVERVIEW, WEAK, REUSED, OLD, DUPLICATES }

data class SecurityUiState(
    val tab: SecurityTab = SecurityTab.OVERVIEW,
    val overview: SecurityOverviewRecord? = null,
    val weak: List<CredentialRecord> = emptyList(),
    val old: List<CredentialRecord> = emptyList(),
    val reusedGroups: List<List<CredentialRecord>> = emptyList(),
    val duplicateGroups: List<List<CredentialRecord>> = emptyList(),
    val isLoading: Boolean = true,
)

class SecurityViewModel(private val repository: VaultRepository) : ViewModel() {
    private val _uiState = MutableStateFlow(SecurityUiState())
    val uiState: StateFlow<SecurityUiState> = _uiState

    init {
        refresh()
    }

    fun selectTab(tab: SecurityTab) {
        _uiState.update { it.copy(tab = tab) }
    }

    fun refresh() {
        _uiState.update { it.copy(isLoading = true) }
        viewModelScope.launch {
            val overview = repository.securityOverview().getOrNull()
            val weak = repository.weakCredentials().getOrNull().orEmpty()
            val old = repository.oldCredentials(180).getOrNull().orEmpty()
            val reused = repository.reusedCredentialGroups().getOrNull().orEmpty()
            val duplicates = repository.duplicateCredentialGroups().getOrNull().orEmpty()
            _uiState.update {
                it.copy(
                    overview = overview,
                    weak = weak,
                    old = old,
                    reusedGroups = reused,
                    duplicateGroups = duplicates,
                    isLoading = false,
                )
            }
        }
    }
}
