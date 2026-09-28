package app.keyflow.mobile.ui.screens.security

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.keyflow.mobile.data.VaultRepository
import uniffi.keyflow_mobile.CredentialRecord

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SecurityScreen(vaultRepository: VaultRepository, onBack: () -> Unit, onOpenCredential: (String) -> Unit) {
    val viewModel: SecurityViewModel = viewModel(
        factory = viewModelFactory { initializer { SecurityViewModel(vaultRepository) } },
    )
    val state by viewModel.uiState.collectAsState()

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Security") },
                navigationIcon = { IconButton(onClick = onBack) { Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back") } },
            )
        },
    ) { padding ->
        Column(modifier = Modifier.fillMaxSize().padding(padding)) {
            ScrollableTabRow(selectedTabIndex = state.tab.ordinal) {
                SecurityTab.entries.forEach { tab ->
                    Tab(
                        selected = state.tab == tab,
                        onClick = { viewModel.selectTab(tab) },
                        text = { Text(tab.name.lowercase().replaceFirstChar { it.uppercase() }) },
                    )
                }
            }
            if (state.isLoading) {
                Column(modifier = Modifier.fillMaxSize(), verticalArrangement = Arrangement.Center) {
                    CircularProgressIndicator(modifier = Modifier.padding(24.dp))
                }
                return@Scaffold
            }
            when (state.tab) {
                SecurityTab.OVERVIEW -> OverviewTab(state)
                SecurityTab.WEAK -> CredentialListTab(state.weak, "No weak passwords found.", onOpenCredential)
                SecurityTab.OLD -> CredentialListTab(state.old, "No passwords older than 180 days.", onOpenCredential)
                SecurityTab.REUSED -> GroupedListTab(state.reusedGroups, "No reused passwords found.", onOpenCredential)
                SecurityTab.DUPLICATES -> GroupedListTab(state.duplicateGroups, "No duplicate entries found.", onOpenCredential)
            }
        }
    }
}

@Composable
private fun OverviewTab(state: SecurityUiState) {
    val o = state.overview ?: return
    Column(modifier = Modifier.fillMaxSize().padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        StatRow("Total credentials", o.total.toInt())
        StatRow("Weak passwords", o.weak.toInt())
        StatRow("Reused passwords", o.reused.toInt())
        StatRow("Old passwords (180+ days)", o.old.toInt())
        StatRow("Duplicate entries", o.duplicates.toInt())
        StatRow("Missing passwords", o.missingPassword.toInt())
        Text(
            "Weak = below ~45 bits of estimated entropy. All estimates are computed on this device from the passwords already in your unlocked vault.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun StatRow(label: String, value: Int) {
    androidx.compose.foundation.layout.Row(modifier = Modifier.fillMaxWidth()) {
        Text(label, modifier = Modifier.weight(1f))
        Text(value.toString(), style = MaterialTheme.typography.titleMedium)
    }
}

@Composable
private fun CredentialListTab(credentials: List<CredentialRecord>, emptyText: String, onOpenCredential: (String) -> Unit) {
    if (credentials.isEmpty()) {
        EmptyState(emptyText)
        return
    }
    LazyColumn { items(credentials, key = { it.id }) { CredentialSimpleRow(it, onOpenCredential) } }
}

@Composable
private fun GroupedListTab(groups: List<List<CredentialRecord>>, emptyText: String, onOpenCredential: (String) -> Unit) {
    if (groups.isEmpty()) {
        EmptyState(emptyText)
        return
    }
    LazyColumn {
        groups.forEach { group ->
            item {
                Text(
                    "${group.size} logins",
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(start = 16.dp, top = 12.dp),
                )
            }
            items(group, key = { it.id }) { CredentialSimpleRow(it, onOpenCredential) }
        }
    }
}

@Composable
private fun CredentialSimpleRow(credential: CredentialRecord, onOpenCredential: (String) -> Unit) {
    ListItem(
        modifier = Modifier.fillMaxWidth().clickable(onClick = { onOpenCredential(credential.id) }),
        headlineContent = { Text(credential.name) },
        supportingContent = { Text(credential.username.ifBlank { credential.url }) },
    )
}

@Composable
private fun EmptyState(text: String) {
    Column(modifier = Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.Center) {
        Text(text, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
