package app.keyflow.mobile.ui.screens.vaultlist

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.Shield
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.VpnKey
import androidx.compose.material.icons.outlined.Star
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.data.VaultRepository
import uniffi.keyflow_mobile.CredentialRecord

@Composable
fun VaultListScreen(
    vaultRepository: VaultRepository,
    onOpenCredential: (String) -> Unit,
    onAddCredential: () -> Unit,
    onOpenGenerator: () -> Unit,
    onOpenSecurity: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    val viewModel: VaultListViewModel = viewModel(
        factory = viewModelFactory { initializer { VaultListViewModel(vaultRepository) } },
    )
    val state by viewModel.uiState.collectAsState()

    Scaffold(
        floatingActionButton = {
            FloatingActionButton(onClick = onAddCredential) {
                Icon(Icons.Filled.Add, contentDescription = "Add login")
            }
        },
        bottomBar = {
            NavigationBar {
                NavigationBarItem(selected = true, onClick = {}, icon = { Icon(Icons.Filled.VpnKey, contentDescription = null) }, label = { Text("Vault") })
                NavigationBarItem(selected = false, onClick = onOpenGenerator, icon = { Icon(Icons.Filled.Add, contentDescription = null) }, label = { Text("Generate") })
                NavigationBarItem(selected = false, onClick = onOpenSecurity, icon = { Icon(Icons.Filled.Shield, contentDescription = null) }, label = { Text("Security") })
                NavigationBarItem(selected = false, onClick = onOpenSettings, icon = { Icon(Icons.Filled.Settings, contentDescription = null) }, label = { Text("Settings") })
            }
        },
    ) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding)) {
            Row(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 8.dp)) {
                OutlinedTextField(
                    value = state.query,
                    onValueChange = viewModel::onQueryChange,
                    modifier = Modifier.fillMaxWidth(),
                    placeholder = { Text("Search logins") },
                    leadingIcon = { Icon(Icons.Filled.Search, contentDescription = null) },
                    singleLine = true,
                )
            }
            if (state.visibleCredentials.isEmpty()) {
                Column(
                    modifier = Modifier.fillMaxSize().padding(32.dp),
                    verticalArrangement = Arrangement.Center,
                ) {
                    Text(
                        if (state.allCredentials.isEmpty()) "No logins yet — tap + to add one." else "No logins match your search.",
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            } else {
                LazyColumn(contentPadding = PaddingValues(bottom = 80.dp)) {
                    items(state.visibleCredentials, key = { it.id }) { credential ->
                        CredentialListItem(
                            credential = credential,
                            onClick = { onOpenCredential(credential.id) },
                            onToggleFavorite = { viewModel.toggleFavorite(credential) },
                        )
                        HorizontalDivider()
                    }
                }
            }
        }
    }
}

@Composable
private fun CredentialListItem(credential: CredentialRecord, onClick: () -> Unit, onToggleFavorite: () -> Unit) {
    ListItem(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick),
        headlineContent = { Text(credential.name, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        supportingContent = { Text(credential.username, maxLines = 1, overflow = TextOverflow.Ellipsis) },
        trailingContent = {
            IconButton(onClick = onToggleFavorite) {
                Icon(
                    imageVector = if (credential.favorite) Icons.Filled.Star else Icons.Outlined.Star,
                    contentDescription = if (credential.favorite) "Unfavorite" else "Favorite",
                    tint = if (credential.favorite) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        },
    )
}
